use {
    super::{
        auction_manager::TOTAL_BIDS_PER_AUCTION_EVM,
        ChainTrait,
        Service,
    },
    crate::{
        api::{
            InstructionError,
            RestError,
            SwapInstructionError,
        },
        auction::{
            entities::{
                self,
                BidChainData,
                BidChainDataCreateSvm,
                BidChainDataSwapCreateSvm,
                BidPaymentInstructionType,
                SubmitType,
            },
            service::get_pending_bids::GetLiveBidsInput,
        },
        kernel::{
            contracts::{
                ExpressRelayContractEvm,
                ExpressRelayErrors,
                MulticallData,
                MulticallStatus,
            },
            entities::{
                Evm,
                PermissionKey,
                Svm,
            },
            traced_client::TracedClient,
        },
        opportunity::{
            self as opportunity,
            entities::{
                get_swap_quote_tokens,
                OpportunitySvm,
                OpportunitySvmProgram::Swap,
                QuoteTokens,
            },
            service::{
                get_live_opportunities::GetLiveOpportunitiesInput,
                get_opportunities::GetLiveOpportunityByIdInput,
                get_quote::get_quote_virtual_permission_account,
            },
        },
    },
    ::express_relay::{
        self as express_relay_svm,
        FeeToken,
    },
    anchor_lang::{
        AnchorDeserialize,
        Discriminator,
    },
    axum::async_trait,
    borsh::de::BorshDeserialize,
    ethers::{
        contract::{
            ContractError,
            ContractRevert,
            FunctionCall,
        },
        middleware::GasOracle,
        providers::Provider,
        signers::Signer,
        types::{
            BlockNumber,
            U256,
        },
    },
    litesvm::types::FailedTransactionMetadata,
    solana_sdk::{
        address_lookup_table::state::AddressLookupTable,
        clock::Slot,
        commitment_config::CommitmentConfig,
        compute_budget,
        instruction::CompiledInstruction,
        pubkey::Pubkey,
        signature::Signature,
        signer::Signer as _,
        system_instruction::SystemInstruction,
        system_program,
        transaction::VersionedTransaction,
    },
    spl_associated_token_account::{
        get_associated_token_address,
        get_associated_token_address_with_program_id,
        instruction::AssociatedTokenAccountInstruction,
    },
    spl_token::instruction::TokenInstruction,
    std::{
        sync::Arc,
        time::Duration,
    },
    time::OffsetDateTime,
    uuid::Uuid,
};

pub struct VerifyBidInput<T: ChainTrait> {
    pub bid_create: entities::BidCreate<T>,
}

pub type VerificationResult<T> = (
    <T as ChainTrait>::BidChainDataType,
    <T as ChainTrait>::BidAmountType,
);

#[async_trait]
pub trait Verification<T: ChainTrait> {
    /// Verify the bid, and extract the chain data from the bid.
    async fn verify_bid(
        &self,
        input: VerifyBidInput<T>,
    ) -> Result<VerificationResult<T>, RestError>;
}

#[derive(Debug, Clone)]
pub struct SwapAccounts {
    pub searcher:               Pubkey,
    pub user_wallet:            Pubkey,
    pub mint_searcher:          Pubkey,
    pub mint_user:              Pubkey,
    pub router_token_account:   Pubkey,
    pub token_program_searcher: Pubkey,
    pub token_program_user:     Pubkey,
}

#[derive(Debug, Clone)]
struct TransferInstructionData {
    from:     Pubkey,
    to:       Pubkey,
    lamports: u64,
}

#[derive(Debug, Clone)]
struct CloseAccountInstructionData {
    account:     Pubkey,
    destination: Pubkey,
    owner:       Pubkey,
}

impl Service<Evm> {
    pub fn get_simulation_call(
        &self,
        permission_key: PermissionKey,
        multicall_data: Vec<MulticallData>,
    ) -> FunctionCall<Arc<Provider<TracedClient>>, Provider<TracedClient>, Vec<MulticallStatus>>
    {
        let client = Arc::new(self.config.chain_config.provider.clone());
        let express_relay_contract = ExpressRelayContractEvm::new(
            self.config.chain_config.express_relay.contract_address,
            client,
        );

        express_relay_contract
            .multicall(permission_key, multicall_data)
            .from(self.config.chain_config.express_relay.relayer.address())
            .block(BlockNumber::Pending)
    }

    // For now, we are only supporting the EIP1559 enabled networks
    async fn verify_bid_exceeds_gas_cost(
        &self,
        estimated_gas: U256,
        bid_amount: U256,
    ) -> Result<(), RestError> {
        let (maximum_gas_fee, priority_fee) = self
            .config
            .chain_config
            .oracle
            .estimate_eip1559_fees()
            .await
            .map_err(|_| RestError::TemporarilyUnavailable)?;

        // To submit TOTAL_BIDS_PER_AUCTION together, each bid must cover the gas fee for all of the submitted bids.
        // To make sure we cover the estimation errors, we add the priority_fee to the final potential gas fee.
        // Therefore, the bid amount needs to be TOTAL_BIDS_PER_AUCTION times per potential gas fee.
        let potential_gas_fee =
            maximum_gas_fee * U256::from(TOTAL_BIDS_PER_AUCTION_EVM) + priority_fee;
        let minimum_bid_amount = potential_gas_fee * estimated_gas;

        if bid_amount >= minimum_bid_amount {
            Ok(())
        } else {
            tracing::info!(
                estimated_gas = estimated_gas.to_string(),
                maximum_gas_fee = maximum_gas_fee.to_string(),
                priority_fee = priority_fee.to_string(),
                minimum_bid_amount = minimum_bid_amount.to_string(),
                "Bid amount is too low"
            );
            Err(RestError::BadParameters(format!(
                "Insufficient bid amount based on the current gas fees. estimated gas usage: {}, maximum fee per gas: {}, priority fee per gas: {}, minimum bid amount: {}",
                estimated_gas, maximum_gas_fee, priority_fee, minimum_bid_amount
            )))
        }
    }

    async fn verify_bid_under_gas_limit(
        &self,
        estimated_gas: U256,
        multiplier: U256,
    ) -> Result<(), RestError> {
        let gas_limit = self.config.chain_config.block_gas_limit;
        if gas_limit < estimated_gas * multiplier {
            let maximum_allowed_gas = gas_limit / multiplier;
            tracing::info!(
                estimated_gas = estimated_gas.to_string(),
                maximum_allowed_gas = maximum_allowed_gas.to_string(),
                "Bid gas usage is too high"
            );
            Err(RestError::BadParameters(format!(
                "Bid estimated gas usage is higher than maximum gas allowed. estimated gas usage: {}, maximum gas allowed: {}",
                estimated_gas, maximum_allowed_gas
            )))
        } else {
            Ok(())
        }
    }
}

#[async_trait]
impl Verification<Evm> for Service<Evm> {
    // As we submit bids together for an auction, the bid is limited as follows:
    // 1. The bid amount should cover gas fees for all bids included in the submission.
    // 2. Depending on the maximum number of bids in the auction, the transaction size for the bid is limited.
    // 3. Depending on the maximum number of bids in the auction, the gas consumption for the bid is limited.
    async fn verify_bid(
        &self,
        input: VerifyBidInput<Evm>,
    ) -> Result<VerificationResult<Evm>, RestError> {
        let bid = input.bid_create;
        tracing::Span::current()
            .record("permission_key", bid.chain_data.permission_key.to_string());
        let call = self.get_simulation_call(
            bid.chain_data.permission_key.clone(),
            vec![MulticallData::from((
                Uuid::new_v4().into_bytes(),
                bid.chain_data.target_contract,
                bid.chain_data.target_calldata.clone(),
                bid.chain_data.amount,
                U256::max_value(),
                // The gas estimation use some binary search algorithm to find the gas limit.
                // It reduce the upper bound threshold on success and increase the lower bound on revert.
                // If the contract does not reverts, the gas estimation will not be accurate in case of external call failures.
                // So we need to make sure in order to calculate the gas estimation correctly, the contract will revert if the external call fails.
                true,
            ))],
        );

        match call.clone().await {
            Ok(results) => {
                if !results[0].external_success {
                    // The call should be reverted because the "revert_on_failure" is set to true.
                    tracing::error!("Simulation failed and call is not reverted: {:?}", results,);
                    return Err(RestError::SimulationError {
                        result: results[0].external_result.clone(),
                        reason: results[0].multicall_revert_reason.clone(),
                    });
                }
            }
            Err(e) => {
                tracing::warn!("Error while simulating bid: {:?}", e);
                return match e {
                    ContractError::Revert(reason) => {
                        if let Some(ExpressRelayErrors::ExternalCallFailed(failure_result)) =
                            ExpressRelayErrors::decode_with_selector(&reason)
                        {
                            return Err(RestError::SimulationError {
                                result: failure_result.status.external_result,
                                reason: failure_result.status.multicall_revert_reason,
                            });
                        }
                        Err(RestError::BadParameters(format!(
                            "Contract Revert Error: {}",
                            reason,
                        )))
                    }
                    ContractError::MiddlewareError { e: _ } => {
                        Err(RestError::TemporarilyUnavailable)
                    }
                    ContractError::ProviderError { e: _ } => Err(RestError::TemporarilyUnavailable),
                    _ => Err(RestError::BadParameters(format!("Error: {}", e))),
                };
            }
        }

        let estimated_gas = call.estimate_gas().await.map_err(|e| {
            tracing::error!("Error while estimating gas: {:?}", e);
            RestError::TemporarilyUnavailable
        })?;

        self.verify_bid_exceeds_gas_cost(estimated_gas, bid.chain_data.amount)
            .await?;
        // The transaction body size will be automatically limited when the gas is limited.
        self.verify_bid_under_gas_limit(estimated_gas, U256::from(TOTAL_BIDS_PER_AUCTION_EVM))
            .await?;

        Ok((
            entities::BidChainDataEvm {
                permission_key:  bid.chain_data.permission_key,
                target_contract: bid.chain_data.target_contract,
                target_calldata: bid.chain_data.target_calldata,
                gas_limit:       estimated_gas,
            },
            bid.chain_data.amount,
        ))
    }
}

pub struct BidDataSvm {
    pub amount:             u64,
    pub router:             Pubkey,
    pub permission_account: Pubkey,
    pub deadline:           OffsetDateTime,
    pub submit_type:        SubmitType,
}

const BID_MINIMUM_LIFE_TIME_SVM_SERVER: Duration = Duration::from_secs(5);
const BID_MINIMUM_LIFE_TIME_SVM_OTHER: Duration = Duration::from_secs(10);

impl Service<Svm> {
    //TODO: merge this logic with simulator logic
    async fn query_lookup_table(&self, table: &Pubkey, index: usize) -> Result<Pubkey, RestError> {
        if let Some(addresses) = self.repo.get_lookup_table(table).await {
            if let Some(account) = addresses.get(index) {
                return Ok(*account);
            }
        }

        let table_data = self
            .config
            .chain_config
            .client
            .get_account_with_commitment(table, CommitmentConfig::processed())
            .await
            .map_err(|e| {
                tracing::error!(error = e.to_string(), "Failed to get lookup table account");
                RestError::TemporarilyUnavailable
            })?
            .value
            .ok_or_else(|| {
                RestError::BadParameters(format!("Lookup table account {} not found", table))
            })?;

        let table_data_deserialized =
            AddressLookupTable::deserialize(&table_data.data).map_err(|e| {
                tracing::warn!(
                    error = e.to_string(),
                    "Failed to deserialize lookup table account data"
                );
                RestError::BadParameters(format!(
                    "Failed deserializing lookup table account data: {}",
                    e
                ))
            })?;

        let account = table_data_deserialized
            .addresses
            .get(index)
            .ok_or_else(|| {
                RestError::BadParameters("Account not found in lookup table".to_string())
            })?;

        self.repo
            .add_lookup_table(*table, table_data_deserialized.addresses.to_vec())
            .await;
        Ok(*account)
    }

    async fn find_and_query_lookup_table(
        &self,
        lookup_accounts: Vec<(Pubkey, u8)>,
        account_position: usize,
    ) -> Result<Pubkey, RestError> {
        let (table_to_query, index_to_query) =
            lookup_accounts.get(account_position).ok_or_else(|| {
                RestError::BadParameters("Lookup table not found in lookup accounts".to_string())
            })?;

        self.query_lookup_table(table_to_query, *index_to_query as usize)
            .await
    }

    async fn extract_account(
        &self,
        tx: &VersionedTransaction,
        instruction: &CompiledInstruction,
        position: usize,
    ) -> Result<Pubkey, RestError> {
        let static_accounts = tx.message.static_account_keys();
        let tx_lookup_tables = tx.message.address_table_lookups();

        let account_position = instruction.accounts.get(position).ok_or_else(|| {
            RestError::BadParameters("Account not found in instruction".to_string())
        })?;

        let account_position: usize = (*account_position).into();
        if let Some(account) = static_accounts.get(account_position) {
            return Ok(*account);
        }

        match tx_lookup_tables {
            Some(tx_lookup_tables) => {
                let lookup_accounts: Vec<(Pubkey, u8)> = tx_lookup_tables
                    .iter()
                    .flat_map(|x| {
                        x.writable_indexes
                            .clone()
                            .into_iter()
                            .map(|y| (x.account_key, y))
                    })
                    .chain(tx_lookup_tables.iter().flat_map(|x| {
                        x.readonly_indexes
                            .clone()
                            .into_iter()
                            .map(|y| (x.account_key, y))
                    }))
                    .collect();

                let account_position_lookups = account_position - static_accounts.len();
                self.find_and_query_lookup_table(lookup_accounts, account_position_lookups)
                    .await
            }
            None => Err(RestError::BadParameters(
                "No lookup tables found in submit_bid instruction".to_string(),
            )),
        }
    }

    pub fn extract_submit_bid_data(
        instruction: &CompiledInstruction,
    ) -> Result<express_relay_svm::SubmitBidArgs, RestError> {
        let discriminator = express_relay_svm::instruction::SubmitBid::DISCRIMINATOR;
        express_relay_svm::SubmitBidArgs::try_from_slice(
            &instruction.data.as_slice()[discriminator.len()..],
        )
        .map_err(|e| {
            RestError::BadParameters(format!("Invalid submit_bid instruction data: {}", e))
        })
    }

    pub fn extract_swap_data(
        instruction: &CompiledInstruction,
    ) -> Result<express_relay_svm::SwapArgs, RestError> {
        let discriminator = express_relay_svm::instruction::Swap::DISCRIMINATOR;
        express_relay_svm::SwapArgs::try_from_slice(
            &instruction.data.as_slice()[discriminator.len()..],
        )
        .map_err(|e| RestError::BadParameters(format!("Invalid swap instruction data: {}", e)))
    }

    pub fn extract_express_relay_instruction(
        &self,
        transaction: VersionedTransaction,
        instruction_type: BidPaymentInstructionType,
    ) -> Result<CompiledInstruction, RestError> {
        let discriminator = match instruction_type {
            BidPaymentInstructionType::SubmitBid => {
                express_relay_svm::instruction::SubmitBid::DISCRIMINATOR
            }
            BidPaymentInstructionType::Swap => express_relay_svm::instruction::Swap::DISCRIMINATOR,
        };
        let instructions = transaction
            .message
            .instructions()
            .iter()
            .filter(|instruction| {
                let program_id = transaction
                    .message
                    .static_account_keys()
                    .get(instruction.program_id_index as usize);
                program_id == Some(&self.config.chain_config.express_relay.program_id)
            })
            .cloned()
            .collect::<Vec<CompiledInstruction>>();

        let instruction = match instructions.len() {
            1 => Ok(instructions[0].clone()),
            _ => Err(RestError::InvalidExpressRelayInstructionCount(
                instructions.len(),
            )),
        }?;
        if !instruction.data.starts_with(discriminator) {
            return Err(RestError::BadParameters(
                "Wrong instruction type for Express Relay Program".to_string(),
            ));
        }
        Ok(instruction)
    }

    async fn check_deadline(
        &self,
        submit_type: &SubmitType,
        deadline: OffsetDateTime,
    ) -> Result<(), RestError> {
        let minimum_bid_life_time = match submit_type {
            SubmitType::ByServer => Some(BID_MINIMUM_LIFE_TIME_SVM_SERVER),
            SubmitType::ByOther => Some(BID_MINIMUM_LIFE_TIME_SVM_OTHER),
            SubmitType::Invalid => None,
        };

        match minimum_bid_life_time {
            Some(min_life_time) => {
                let minimum_deadline = OffsetDateTime::now_utc() + min_life_time;
                // TODO: this uses the time at the server, which can lead to issues if Solana ever experiences clock drift
                // using the time at the server is not ideal, but the alternative is to make an RPC call to get the Solana block time
                // we should make this more robust, possibly by polling the current block time in the background
                if deadline < minimum_deadline {
                    return Err(RestError::InvalidDeadline {
                        deadline,
                        minimum: min_life_time,
                    });
                }

                Ok(())
            }
            None => Ok(()),
        }
    }

    fn validate_swap_transaction_instructions(
        &self,
        tx: &VersionedTransaction,
    ) -> Result<(), RestError> {
        tx.message
            .instructions()
            .iter()
            .enumerate()
            .try_for_each(|(index, ix)| {
                self.validate_swap_transaction_instruction(tx.message.static_account_keys(), ix)
                    .map_err(|e| RestError::InvalidInstruction(Some(index), e))
            })?;

        Ok(())
    }

    fn validate_swap_transaction_instruction(
        &self,
        accounts: &[Pubkey],
        ix: &CompiledInstruction,
    ) -> Result<(), InstructionError> {
        let program_id = accounts
            .get(ix.program_id_index as usize)
            .ok_or(InstructionError::ProgramIdIndexOutOfBounds)?;

        if *program_id == system_program::id() {
            if Self::is_system_program_transfer_instruction(ix, accounts) {
                Ok(())
            } else {
                Err(InstructionError::UnsupportedSystemProgramInstruction)
            }
        } else if *program_id == spl_token::id() {
            let ix_parsed = TokenInstruction::unpack(&ix.data)
                .map_err(InstructionError::InvalidSplTokenInstruction)?;
            match ix_parsed {
                TokenInstruction::CloseAccount { .. } => Ok(()),
                TokenInstruction::SyncNative { .. } => Ok(()),
                _ => Err(InstructionError::UnsupportedSplTokenInstruction(format!(
                    "{:?}",
                    ix_parsed
                ))),
            }
        } else if *program_id == compute_budget::id() {
            Ok(())
        } else if *program_id == spl_associated_token_account::id() {
            let ix_parsed =
                AssociatedTokenAccountInstruction::try_from_slice(&ix.data).map_err(|e| {
                    InstructionError::InvalidAssociatedTokenAccountInstruction(e.to_string())
                })?;
            match ix_parsed {
                AssociatedTokenAccountInstruction::Create => Ok(()),
                AssociatedTokenAccountInstruction::CreateIdempotent => Ok(()),
                _ => Err(InstructionError::UnsupportedAssociatedTokenAccountInstruction(ix_parsed)),
            }
        } else if *program_id == self.config.chain_config.express_relay.program_id {
            Ok(())
        } else {
            Err(InstructionError::UnsupportedProgram(*program_id))
        }
    }

    async fn check_svm_swap_bid_fields(
        &self,
        bid_data: &BidChainDataSwapCreateSvm,
        opp: &OpportunitySvm,
    ) -> Result<(), RestError> {
        let swap_instruction = self.extract_express_relay_instruction(
            bid_data.transaction.clone(),
            BidPaymentInstructionType::Swap,
        )?;
        let swap_data = Self::extract_swap_data(&swap_instruction)?;
        let SwapAccounts {
            user_wallet,
            mint_searcher,
            mint_user,
            token_program_searcher,
            token_program_user,
            ..
        } = self
            .extract_swap_accounts(&bid_data.transaction, &swap_instruction)
            .await?;
        let quote_tokens = get_swap_quote_tokens(opp);
        let opp_swap_data = match &opp.program {
            Swap(opp_swap_data) => opp_swap_data,
            _ => {
                return Err(RestError::BadParameters(format!(
                    "Opportunity with id {} is not a swap opportunity",
                    bid_data.opportunity_id
                )));
            }
        };
        let (
            expected_mint_user,
            expected_amount_user,
            expected_mint_searcher,
            expected_amount_searcher,
        ) = match quote_tokens.clone() {
            QuoteTokens::UserTokenSpecified {
                user_token,
                searcher_token,
                ..
            } => (
                user_token.token,
                Some(user_token.amount),
                searcher_token,
                None,
            ),
            QuoteTokens::SearcherTokenSpecified {
                user_token,
                searcher_token,
                ..
            } => (
                user_token,
                None,
                searcher_token.token,
                Some(searcher_token.amount),
            ),
        };
        if user_wallet != opp_swap_data.user_wallet_address {
            return Err(RestError::InvalidSwapInstruction(
                SwapInstructionError::UserWalletAddress {
                    expected: opp_swap_data.user_wallet_address,
                    found:    user_wallet,
                },
            ));
        }
        if expected_mint_searcher != mint_searcher {
            return Err(RestError::InvalidSwapInstruction(
                SwapInstructionError::MintSearcher {
                    expected: expected_mint_searcher,
                    found:    mint_searcher,
                },
            ));
        }
        if expected_mint_user != mint_user {
            return Err(RestError::InvalidSwapInstruction(
                SwapInstructionError::MintUser {
                    expected: expected_mint_user,
                    found:    mint_user,
                },
            ));
        }

        if token_program_searcher != opp_swap_data.token_program_searcher {
            return Err(RestError::InvalidSwapInstruction(
                SwapInstructionError::TokenProgramSearcher {
                    expected: opp_swap_data.token_program_searcher,
                    found:    token_program_searcher,
                },
            ));
        }

        if token_program_user != opp_swap_data.token_program_user {
            return Err(RestError::InvalidSwapInstruction(
                SwapInstructionError::TokenProgramUser {
                    expected: opp_swap_data.token_program_user,
                    found:    token_program_user,
                },
            ));
        }

        if let Some(expected_amount_searcher) = expected_amount_searcher {
            if expected_amount_searcher != swap_data.amount_searcher {
                return Err(RestError::InvalidSwapInstruction(
                    SwapInstructionError::AmountSearcher {
                        expected: expected_amount_searcher,
                        found:    swap_data.amount_searcher,
                    },
                ));
            }
        }
        if let Some(expected_amount_user) = expected_amount_user {
            if expected_amount_user != swap_data.amount_user {
                return Err(RestError::InvalidSwapInstruction(
                    SwapInstructionError::AmountUser {
                        expected: expected_amount_user,
                        found:    swap_data.amount_user,
                    },
                ));
            }
        }
        if opp_swap_data.fee_token != swap_data.fee_token {
            return Err(RestError::InvalidSwapInstruction(
                SwapInstructionError::FeeToken {
                    expected: opp_swap_data.fee_token.clone(),
                    found:    swap_data.fee_token,
                },
            ));
        }

        if swap_data.referral_fee_bps != opp_swap_data.referral_fee_bps {
            return Err(RestError::InvalidSwapInstruction(
                SwapInstructionError::ReferralFee {
                    expected: opp_swap_data.referral_fee_bps,
                    found:    swap_data.referral_fee_bps,
                },
            ));
        }
        Ok(())
    }

    pub async fn extract_swap_accounts(
        &self,
        tx: &VersionedTransaction,
        swap_instruction: &CompiledInstruction,
    ) -> Result<SwapAccounts, RestError> {
        let positions = &self
            .config
            .chain_config
            .express_relay
            .swap_instruction_account_positions;

        let searcher = self
            .extract_account(tx, swap_instruction, positions.searcher_account)
            .await?;
        let user_wallet = self
            .extract_account(tx, swap_instruction, positions.user_wallet_account)
            .await?;
        let mint_searcher = self
            .extract_account(tx, swap_instruction, positions.mint_searcher_account)
            .await?;
        let mint_user = self
            .extract_account(tx, swap_instruction, positions.mint_user_account)
            .await?;
        let router_token_account = self
            .extract_account(tx, swap_instruction, positions.router_token_account)
            .await?;
        let token_program_searcher = self
            .extract_account(tx, swap_instruction, positions.token_program_searcher)
            .await?;
        let token_program_user = self
            .extract_account(tx, swap_instruction, positions.token_program_user)
            .await?;

        Ok(SwapAccounts {
            searcher,
            user_wallet,
            mint_searcher,
            mint_user,
            router_token_account,
            token_program_searcher,
            token_program_user,
        })
    }

    fn is_system_program_transfer_instruction(
        instruction: &CompiledInstruction,
        accounts: &[Pubkey],
    ) -> bool {
        let program_id = accounts.get(instruction.program_id_index as usize);
        if program_id != Some(&system_program::id()) {
            return false;
        }

        matches!(
            bincode::deserialize::<SystemInstruction>(&instruction.data),
            Ok(SystemInstruction::Transfer { .. })
        )
    }

    fn extract_transfer_instructions(
        tx: &VersionedTransaction,
    ) -> Result<Vec<TransferInstructionData>, RestError> {
        let instructions: Vec<&CompiledInstruction> = tx
            .message
            .instructions()
            .iter()
            .filter(|instruction| {
                Self::is_system_program_transfer_instruction(
                    instruction,
                    tx.message.static_account_keys(),
                )
            })
            .collect();
        let mut result = vec![];
        for instruction in instructions {
            let data =
                bincode::deserialize::<SystemInstruction>(&instruction.data).map_err(|_| {
                    RestError::BadParameters("Invalid sol transfer instruction data".to_string())
                })?;
            let transfer_instruction = match data {
                SystemInstruction::Transfer { lamports } => {
                    if instruction.accounts.len() != 2 {
                        return Err(RestError::BadParameters(
                            "Invalid sol transfer instruction accounts".to_string(),
                        ));
                    }
                    TransferInstructionData {
                        from: *tx
                            .message
                            .static_account_keys()
                            .get(instruction.accounts[0] as usize)
                            .ok_or(RestError::BadParameters(
                                "Invalid account in sol transfer instruction".to_string(),
                            ))?,
                        to: *tx
                            .message
                            .static_account_keys()
                            .get(instruction.accounts[1] as usize)
                            .ok_or(RestError::BadParameters(
                                "Invalid account in sol transfer instruction".to_string(),
                            ))?,
                        lamports,
                    }
                }
                _ => {
                    return Err(RestError::BadParameters(
                        "Invalid sol transfer instruction data".to_string(),
                    ))
                }
            };
            result.push(transfer_instruction);
        }
        Ok(result)
    }

    fn check_transfer_instruction(
        tx: &VersionedTransaction,
        swap_data: &express_relay_svm::SwapArgs,
        swap_accounts: &SwapAccounts,
    ) -> Result<(), RestError> {
        let transfer_instructions = Self::extract_transfer_instructions(tx)?;
        if transfer_instructions.len() > 1 {
            return Err(RestError::InvalidInstruction(
                None,
                InstructionError::InvalidTransferInstructionsCount,
            ));
        }

        // User have to wrap Sol
        if swap_accounts.mint_user == spl_token::native_mint::id() {
            if transfer_instructions.len() != 1 {
                return Err(RestError::InvalidInstruction(
                    None,
                    InstructionError::InvalidTransferInstructionsCount,
                ));
            }
            let transfer_instruction = transfer_instructions[0].clone();
            let user_ata = get_associated_token_address(
                &swap_accounts.user_wallet,
                &spl_token::native_mint::id(),
            );
            if transfer_instruction.from != swap_accounts.user_wallet {
                return Err(RestError::InvalidInstruction(
                    None,
                    InstructionError::InvalidFromAccountTransferInstruction {
                        expected: swap_accounts.user_wallet,
                        found:    transfer_instruction.from,
                    },
                ));
            }
            if transfer_instruction.to != user_ata {
                return Err(RestError::InvalidInstruction(
                    None,
                    InstructionError::InvalidToAccountTransferInstruction {
                        expected: user_ata,
                        found:    transfer_instruction.to,
                    },
                ));
            }
            if swap_data.amount_user != transfer_instruction.lamports {
                return Err(RestError::InvalidInstruction(
                    None,
                    InstructionError::InvalidAmountTransferInstruction {
                        expected: swap_data.amount_user,
                        found:    transfer_instruction.lamports,
                    },
                ));
            }
        }
        // Searcher may want to wrap Sol
        // We dont care about the amount here
        else if swap_accounts.mint_searcher == spl_token::native_mint::id()
            && transfer_instructions.len() == 1
        {
            let transfer_instruction = transfer_instructions[0].clone();
            let searcher_ata = get_associated_token_address(
                &swap_accounts.searcher,
                &spl_token::native_mint::id(),
            );
            if transfer_instruction.from != swap_accounts.searcher {
                return Err(RestError::InvalidInstruction(
                    None,
                    InstructionError::InvalidFromAccountTransferInstruction {
                        expected: swap_accounts.searcher,
                        found:    transfer_instruction.from,
                    },
                ));
            }
            if transfer_instruction.to != searcher_ata {
                return Err(RestError::InvalidInstruction(
                    None,
                    InstructionError::InvalidToAccountTransferInstruction {
                        expected: searcher_ata,
                        found:    transfer_instruction.to,
                    },
                ));
            }
        }
        // No transfer instruction is allowed
        else if !transfer_instructions.is_empty() {
            return Err(RestError::InvalidInstruction(
                None,
                InstructionError::TransferInstructionNotAllowed,
            ));
        }

        Ok(())
    }

    fn extract_token_instructions(tx: &VersionedTransaction) -> Vec<&CompiledInstruction> {
        tx.message
            .instructions()
            .iter()
            .filter(|instruction| {
                let program_id = tx
                    .message
                    .static_account_keys()
                    .get(instruction.program_id_index as usize);
                program_id == Some(&spl_token::id())
            })
            .collect()
    }

    fn extract_sync_native_instructions(tx: &VersionedTransaction) -> Vec<&CompiledInstruction> {
        let token_instructions = Self::extract_token_instructions(tx);
        token_instructions
            .into_iter()
            .filter(|instruction| {
                let ix_parsed = TokenInstruction::unpack(&instruction.data).ok();
                matches!(ix_parsed, Some(TokenInstruction::SyncNative))
            })
            .collect()
    }

    fn check_sync_native_instruction_exists(
        tx: &VersionedTransaction,
        wallet_address: &Pubkey,
    ) -> Result<(), RestError> {
        let sync_native_instructions = Self::extract_sync_native_instructions(tx);
        let ata = get_associated_token_address(wallet_address, &spl_token::native_mint::id());

        if sync_native_instructions
            .iter()
            .filter(|instruction| {
                if instruction.accounts.len() == 1 {
                    tx.message
                        .static_account_keys()
                        .get(instruction.accounts[0] as usize)
                        == Some(&ata)
                } else {
                    false
                }
            })
            .count()
            != 1
        {
            return Err(RestError::InvalidInstruction(
                None,
                InstructionError::InvalidSyncNativeInstructionCount(ata),
            ));
        }

        Ok(())
    }

    fn extract_close_account_instructions(
        tx: &VersionedTransaction,
    ) -> Result<Vec<CloseAccountInstructionData>, RestError> {
        let mut result = vec![];
        for instruction in Self::extract_token_instructions(tx) {
            let ix_parsed = TokenInstruction::unpack(&instruction.data).ok();
            if let Some(TokenInstruction::CloseAccount) = ix_parsed {
                if instruction.accounts.len() < 3 {
                    return Err(RestError::BadParameters(
                        "Invalid close account instruction accounts".to_string(),
                    ));
                }
                let invalid_account_message =
                    "Invalid account in close account instruction".to_string();
                let account_to_close = tx
                    .message
                    .static_account_keys()
                    .get(instruction.accounts[0] as usize)
                    .ok_or(RestError::BadParameters(invalid_account_message.clone()))?;
                let destination = tx
                    .message
                    .static_account_keys()
                    .get(instruction.accounts[1] as usize)
                    .ok_or(RestError::BadParameters(invalid_account_message.clone()))?;
                let owner = tx
                    .message
                    .static_account_keys()
                    .get(instruction.accounts[2] as usize)
                    .ok_or(RestError::BadParameters(invalid_account_message))?;
                result.push(CloseAccountInstructionData {
                    account:     *account_to_close,
                    destination: *destination,
                    owner:       *owner,
                });
            }
        }
        Ok(result)
    }

    fn check_close_account_instruction(
        tx: &VersionedTransaction,
        swap_accounts: &SwapAccounts,
    ) -> Result<(), RestError> {
        let close_account_instructions = Self::extract_close_account_instructions(tx)?;
        if close_account_instructions.len() > 2 {
            return Err(RestError::InvalidInstruction(
                None,
                InstructionError::InvalidCloseAccountInstructionsCount,
            ));
        }

        let (user_unwrap_sol_instructions, searcher_unwrap_sol_instructions): (
            Vec<CloseAccountInstructionData>,
            Vec<CloseAccountInstructionData>,
        ) = close_account_instructions
            .into_iter()
            .partition(|instruction| {
                instruction.account
                    == get_associated_token_address(
                        &swap_accounts.user_wallet,
                        &spl_token::native_mint::id(),
                    )
            });

        // User has to unwrap Sol
        if swap_accounts.mint_searcher == spl_token::native_mint::id()
            || swap_accounts.mint_user == spl_token::native_mint::id()
        {
            if user_unwrap_sol_instructions.len() != 1 {
                return Err(RestError::InvalidInstruction(
                    None,
                    InstructionError::InvalidCloseAccountInstructionsCount,
                ));
            }
            let close_account_instruction = user_unwrap_sol_instructions[0].clone();
            let ata = get_associated_token_address(
                &swap_accounts.user_wallet,
                &spl_token::native_mint::id(),
            );
            if close_account_instruction.account != ata {
                return Err(RestError::InvalidInstruction(
                    None,
                    InstructionError::InvalidAccountToCloseCloseAccountInstruction {
                        expected: ata,
                        found:    close_account_instruction.account,
                    },
                ));
            }
            if close_account_instruction.destination != swap_accounts.user_wallet {
                return Err(RestError::InvalidInstruction(
                    None,
                    InstructionError::InvalidDestinationCloseAccountInstruction {
                        expected: swap_accounts.user_wallet,
                        found:    close_account_instruction.destination,
                    },
                ));
            }
            if close_account_instruction.owner != swap_accounts.user_wallet {
                return Err(RestError::InvalidInstruction(
                    None,
                    InstructionError::InvalidOwnerCloseAccountInstruction {
                        expected: swap_accounts.user_wallet,
                        found:    close_account_instruction.owner,
                    },
                ));
            }

            // Searcher may want to unwrap Sol
            // We dont care about destination and owner in this case
            if searcher_unwrap_sol_instructions.len() == 1 {
                let close_account_instruction = searcher_unwrap_sol_instructions[0].clone();
                let ata = get_associated_token_address(
                    &swap_accounts.searcher,
                    &spl_token::native_mint::id(),
                );
                if close_account_instruction.account != ata {
                    return Err(RestError::InvalidInstruction(
                        None,
                        InstructionError::InvalidAccountToCloseCloseAccountInstruction {
                            expected: ata,
                            found:    close_account_instruction.account,
                        },
                    ));
                }
            }
        } else if !user_unwrap_sol_instructions.is_empty()
            || !searcher_unwrap_sol_instructions.is_empty()
        {
            return Err(RestError::InvalidInstruction(
                None,
                InstructionError::CloseAccountInstructionNotAllowed,
            ));
        }

        Ok(())
    }

    fn check_wrap_unwrap_native_token_instructions(
        &self,
        tx: &VersionedTransaction,
        swap_data: &express_relay_svm::SwapArgs,
        swap_accounts: &SwapAccounts,
    ) -> Result<(), RestError> {
        Self::check_transfer_instruction(tx, swap_data, swap_accounts)?;
        if swap_accounts.mint_user == spl_token::native_mint::id() {
            // User have to wrap Sol
            // So we need to check if there is a sync native instruction
            Self::check_sync_native_instruction_exists(tx, &swap_accounts.user_wallet)?;
        }
        Self::check_close_account_instruction(tx, swap_accounts)?;
        Ok(())
    }

    pub async fn extract_bid_data(
        &self,
        bid_chain_data_create_svm: &BidChainDataCreateSvm,
    ) -> Result<BidDataSvm, RestError> {
        let svm_config = &self.config.chain_config.express_relay;
        match bid_chain_data_create_svm {
            BidChainDataCreateSvm::OnChain(bid_data) => {
                let submit_bid_instruction = self.extract_express_relay_instruction(
                    bid_data.transaction.clone(),
                    BidPaymentInstructionType::SubmitBid,
                )?;
                let submit_bid_data = Self::extract_submit_bid_data(&submit_bid_instruction)?;

                let permission_account = self
                    .extract_account(
                        &bid_data.transaction,
                        &submit_bid_instruction,
                        svm_config
                            .submit_bid_instruction_account_positions
                            .permission_account,
                    )
                    .await?;
                let router = self
                    .extract_account(
                        &bid_data.transaction,
                        &submit_bid_instruction,
                        svm_config
                            .submit_bid_instruction_account_positions
                            .router_account,
                    )
                    .await?;
                Ok(BidDataSvm {
                    amount: submit_bid_data.bid_amount,
                    permission_account,
                    router,
                    deadline: OffsetDateTime::from_unix_timestamp(submit_bid_data.deadline)
                        .map_err(|e| {
                            RestError::BadParameters(format!(
                                "Invalid deadline: {:?} {:?}",
                                submit_bid_data.deadline, e
                            ))
                        })?,
                    submit_type: SubmitType::ByServer,
                })
            }
            BidChainDataCreateSvm::Swap(bid_data) => {
                let opp = self
                    .opportunity_service
                    .get_live_opportunity_by_id(GetLiveOpportunityByIdInput {
                        opportunity_id: bid_data.opportunity_id,
                    })
                    .await
                    .ok_or(RestError::SwapOpportunityNotFound)?;
                self.validate_swap_transaction_instructions(
                    bid_chain_data_create_svm.get_transaction(),
                )?;
                self.check_svm_swap_bid_fields(bid_data, &opp).await?;

                let swap_instruction = self.extract_express_relay_instruction(
                    bid_data.transaction.clone(),
                    BidPaymentInstructionType::Swap,
                )?;
                let swap_data = Self::extract_swap_data(&swap_instruction)?;
                let swap_accounts = self
                    .extract_swap_accounts(&bid_data.transaction, &swap_instruction)
                    .await?;
                let SwapAccounts {
                    user_wallet,
                    mint_searcher,
                    mint_user,
                    router_token_account,
                    token_program_searcher,
                    token_program_user,
                    ..
                } = swap_accounts.clone();

                self.check_wrap_unwrap_native_token_instructions(
                    &bid_data.transaction,
                    &swap_data,
                    &swap_accounts,
                )?;

                let quote_tokens = get_swap_quote_tokens(&opp);
                let bid_amount = match quote_tokens.clone() {
                    // bid is in the unspecified token
                    QuoteTokens::UserTokenSpecified { .. } => swap_data.amount_searcher,
                    QuoteTokens::SearcherTokenSpecified { .. } => swap_data.amount_user,
                };
                let (fee_token, fee_token_program) = match swap_data.fee_token {
                    FeeToken::Searcher => (mint_searcher, token_program_searcher),
                    FeeToken::User => (mint_user, token_program_user),
                };
                let expected_router_token_account = get_associated_token_address_with_program_id(
                    &opp.router,
                    &fee_token,
                    &fee_token_program,
                );

                if router_token_account != expected_router_token_account {
                    return Err(RestError::InvalidSwapInstruction(
                        SwapInstructionError::AssociatedRouterTokenAccount {
                            expected: expected_router_token_account,
                            found:    router_token_account,
                        },
                    ));
                }

                let permission_account = get_quote_virtual_permission_account(
                    &quote_tokens,
                    &user_wallet,
                    &router_token_account,
                    swap_data.referral_fee_bps,
                );

                Ok(BidDataSvm {
                    amount: bid_amount,
                    permission_account,
                    router: opp.router,
                    deadline: OffsetDateTime::from_unix_timestamp(swap_data.deadline).map_err(
                        |e| {
                            RestError::BadParameters(format!(
                                "Invalid deadline: {:?} {:?}",
                                swap_data.deadline, e
                            ))
                        },
                    )?,
                    submit_type: SubmitType::ByOther,
                })
            }
        }
    }

    fn relayer_signer_exists(
        &self,
        accounts: &[Pubkey],
        signatures: &[Signature],
    ) -> Result<(), RestError> {
        let relayer_pubkey = self.config.chain_config.express_relay.relayer.pubkey();
        let relayer_exists = accounts[..signatures.len()]
            .iter()
            .any(|account| account.eq(&relayer_pubkey));

        if !relayer_exists {
            return Err(RestError::RelayerNotSigner(relayer_pubkey));
        }
        Ok(())
    }

    fn all_signatures_exists(
        &self,
        message_bytes: &[u8],
        accounts: &[Pubkey],
        signatures: &[Signature],
        missing_signers: &[Pubkey],
    ) -> Result<(), RestError> {
        for (signature, pubkey) in signatures.iter().zip(accounts.iter()) {
            if missing_signers.contains(pubkey) {
                continue;
            }
            if !signature.verify(pubkey.as_ref(), message_bytes) {
                return Err(RestError::InvalidSignature(*pubkey));
            }
        }
        Ok(())
    }

    async fn verify_signatures(
        &self,
        bid: &entities::BidCreate<Svm>,
        chain_data: &entities::BidChainDataSvm,
        submit_type: &SubmitType,
    ) -> Result<(), RestError> {
        let message_bytes = chain_data.transaction.message.serialize();
        let signatures = chain_data.transaction.signatures.clone();
        let accounts = chain_data.transaction.message.static_account_keys();
        let permission_key = chain_data.get_permission_key();
        match submit_type {
            SubmitType::Invalid => {
                // TODO Look at the todo comment in get_quote.rs file in opportunity module
                Err(RestError::BadParameters(format!(
                    "The permission key is not valid for auction anymore: {:?}",
                    permission_key
                )))
            }
            SubmitType::ByOther => {
                let opportunities = self
                    .opportunity_service
                    .get_live_opportunities(GetLiveOpportunitiesInput {
                        key: opportunity::entities::OpportunityKey(
                            bid.chain_id.clone(),
                            PermissionKey::from(permission_key.0),
                        ),
                    })
                    .await;

                let opportunity = opportunities
                    .first()
                    .ok_or_else(|| RestError::BadParameters("Opportunity not found".to_string()))?;
                opportunity
                    .check_fee_payer(accounts)
                    .map_err(|e| RestError::InvalidFirstSigner(e.to_string()))?;
                let mut missing_signers = opportunity.get_missing_signers();
                missing_signers.push(self.config.chain_config.express_relay.relayer.pubkey());
                self.relayer_signer_exists(accounts, &signatures)?;
                self.all_signatures_exists(&message_bytes, accounts, &signatures, &missing_signers)
            }
            SubmitType::ByServer => {
                self.relayer_signer_exists(accounts, &signatures)?;
                self.all_signatures_exists(
                    &message_bytes,
                    accounts,
                    &signatures,
                    &[self.config.chain_config.express_relay.relayer.pubkey()],
                )
            }
        }
    }

    pub async fn simulate_swap_bid(&self, bid: &entities::BidCreate<Svm>) -> Result<(), RestError> {
        let tx = bid.chain_data.get_transaction();
        let simulation = self
            .config
            .chain_config
            .client
            .simulate_transaction(tx)
            .await;
        match simulation {
            Ok(simulation) => {
                if simulation.value.err.is_some() {
                    let msgs = simulation.value.logs.unwrap_or_default();
                    Err(RestError::SimulationError {
                        result: Default::default(),
                        reason: msgs.join("\n"),
                    })
                } else {
                    Ok(())
                }
            }
            Err(e) => {
                tracing::error!("Error while simulating swap bid: {:?}", e);
                Err(RestError::TemporarilyUnavailable)
            }
        }
    }

    pub async fn simulate_bid(&self, bid: &entities::BidCreate<Svm>) -> Result<(), RestError> {
        const RETRY_LIMIT: usize = 5;
        const RETRY_DELAY: Duration = Duration::from_millis(100);
        let mut retry_count = 0;
        let bid_slot = match &bid.chain_data {
            BidChainDataCreateSvm::OnChain(onchain_data) => onchain_data.slot,
            BidChainDataCreateSvm::Swap(_) => None,
        }
        .unwrap_or_default();

        let should_retry = |result_slot: Slot,
                            retry_count: usize,
                            err: &FailedTransactionMetadata|
         -> bool {
            if result_slot < bid_slot && retry_count < RETRY_LIMIT {
                tracing::warn!(
                "Simulation failed with stale slot. Simulation slot: {}, Bid Slot: {}, Retry count: {}, Error: {:?}",
                result_slot,
                bid_slot,
                retry_count,
                err
            );
                true
            } else {
                false
            }
        };

        loop {
            let response = self
                .config
                .chain_config
                .simulator
                .simulate_transaction(bid.chain_data.get_transaction())
                .await;
            let result = response.map_err(|e| {
                tracing::error!("Error while simulating bid: {:?}", e);
                RestError::TemporarilyUnavailable
            })?;
            return match result.value {
                Err(err) => {
                    if should_retry(result.context.slot, retry_count, &err) {
                        tokio::time::sleep(RETRY_DELAY).await;
                        retry_count += 1;
                        continue;
                    }
                    let msgs = err.meta.logs;
                    Err(RestError::SimulationError {
                        result: Default::default(),
                        reason: msgs.join("\n"),
                    })
                }
                // Not important to check if bid slot is less than simulation slot if simulation is successful
                // since we want to fix incorrect verifications due to stale slot
                Ok(_) => Ok(()),
            };
        }
    }

    async fn check_compute_budget(
        &self,
        transaction: &VersionedTransaction,
    ) -> Result<(), RestError> {
        let compute_unit_price = self
            .repo
            .get_priority_fees(OffsetDateTime::now_utc() - Duration::from_secs(15))
            .await
            .iter()
            .map(|sample| sample.fee)
            .min()
            .unwrap_or(0);

        let budgets: Vec<u64> = transaction
            .message
            .instructions()
            .iter()
            .filter_map(|instruction| {
                let program_id = transaction
                    .message
                    .static_account_keys()
                    .get(instruction.program_id_index as usize);
                if program_id != Some(&compute_budget::id()) {
                    return None;
                }

                match compute_budget::ComputeBudgetInstruction::try_from_slice(&instruction.data) {
                    Ok(compute_budget::ComputeBudgetInstruction::SetComputeUnitPrice(price)) => {
                        Some(price)
                    }
                    _ => None,
                }
            })
            .collect();
        if budgets.len() > 1 {
            return Err(RestError::MultipleSetComputeUnitPriceInstructions);
        }
        if budgets.is_empty() && compute_unit_price > 0 {
            return Err(RestError::SetComputeUnitPriceInstructionNotFound(
                compute_unit_price,
            ));
        }
        if let Some(budget) = budgets.first() {
            if *budget < compute_unit_price {
                return Err(RestError::LowComputeUnitPrice(compute_unit_price));
            }
        }
        Ok(())
    }
}

#[async_trait]
impl Verification<Svm> for Service<Svm> {
    async fn verify_bid(
        &self,
        input: VerifyBidInput<Svm>,
    ) -> Result<VerificationResult<Svm>, RestError> {
        let bid = input.bid_create;
        let transaction = bid.chain_data.get_transaction().clone();
        Svm::check_tx_size(&transaction)?;
        self.check_compute_budget(&transaction).await?;
        let bid_data = self.extract_bid_data(&bid.chain_data).await?;
        let bid_payment_instruction_type = match bid_data.submit_type {
            SubmitType::ByServer => BidPaymentInstructionType::SubmitBid,
            SubmitType::ByOther => BidPaymentInstructionType::Swap,
            SubmitType::Invalid => {
                return Err(RestError::BadParameters(
                    "Invalid submit type for bid".to_string(),
                ));
            }
        };
        let bid_chain_data = entities::BidChainDataSvm {
            permission_account:           bid_data.permission_account,
            router:                       bid_data.router,
            bid_payment_instruction_type: bid_payment_instruction_type.clone(),
            transaction:                  transaction.clone(),
        };
        let permission_key = bid_chain_data.get_permission_key();
        tracing::Span::current().record("permission_key", bid_data.permission_account.to_string());
        self.check_deadline(&bid_data.submit_type, bid_data.deadline)
            .await?;
        self.verify_signatures(&bid, &bid_chain_data, &bid_data.submit_type)
            .await?;
        match bid_payment_instruction_type {
            BidPaymentInstructionType::Swap => self.simulate_swap_bid(&bid).await?,
            BidPaymentInstructionType::SubmitBid => self.simulate_bid(&bid).await?,
        }

        // Check if the bid is not duplicate
        let pending_bids = self
            .get_pending_bids(GetLiveBidsInput { permission_key })
            .await;
        if pending_bids.iter().any(|b| bid == *b) {
            return Err(RestError::DuplicateBid);
        }

        Ok((bid_chain_data, bid_data.amount))
    }
}

#[cfg(test)]
mod tests {
    use {
        super::VerificationResult,
        crate::{
            api::{
                InstructionError,
                RestError,
                SwapInstructionError,
            },
            auction::{
                entities::{
                    BidChainDataCreateSvm,
                    BidChainDataSvm,
                    BidChainDataSwapCreateSvm,
                    BidCreate,
                    BidPaymentInstructionType,
                },
                repository::{
                    MockDatabase,
                    Repository,
                },
                service::{
                    verification::{
                        Verification,
                        BID_MINIMUM_LIFE_TIME_SVM_OTHER,
                    },
                    Service,
                },
            },
            kernel::{
                entities::{
                    ChainId,
                    Svm,
                },
                traced_sender_svm::tests::MockRpcClient,
            },
            opportunity::{
                entities::{
                    FeeToken,
                    OpportunityCoreFields,
                    OpportunitySvm,
                    OpportunitySvmProgram,
                    OpportunitySvmProgramSwap,
                    QuoteTokens,
                    TokenAccountInitializationConfig,
                    TokenAccountInitializationConfigs,
                    TokenAmountSvm,
                },
                service::{
                    get_quote::get_quote_virtual_permission_account,
                    ChainTypeSvm,
                    MockService,
                },
            },
        },
        borsh::BorshDeserialize,
        ethers::types::Bytes,
        express_relay_api_types::opportunity as opportunity_api,
        express_relay_client::svm::{
            self,
            GetSubmitBidInstructionParams,
            GetSwapInstructionParams,
        },
        solana_client::{
            nonblocking::rpc_client::RpcClient,
            rpc_client::RpcClientConfig,
        },
        solana_sdk::{
            compute_budget,
            hash::Hash,
            instruction::{
                AccountMeta,
                Instruction,
            },
            packet::PACKET_DATA_SIZE,
            pubkey::Pubkey,
            signature::Keypair,
            signer::Signer,
            system_instruction,
            transaction::Transaction,
        },
        spl_associated_token_account::{
            get_associated_token_address,
            get_associated_token_address_with_program_id,
            instruction::{
                recover_nested,
                AssociatedTokenAccountInstruction,
            },
        },
        spl_token::instruction::TokenInstruction,
        std::sync::Arc,
        time::{
            Duration,
            OffsetDateTime,
        },
        uuid::Uuid,
    };

    impl TokenAccountInitializationConfigs {
        pub fn searcher_payer() -> Self {
            Self {
                user_ata_mint_searcher:         TokenAccountInitializationConfig::SearcherPayer,
                user_ata_mint_user:             TokenAccountInitializationConfig::Unneeded,
                router_fee_receiver_ta:         TokenAccountInitializationConfig::SearcherPayer,
                relayer_fee_receiver_ata:       TokenAccountInitializationConfig::SearcherPayer,
                express_relay_fee_receiver_ata: TokenAccountInitializationConfig::SearcherPayer,
            }
        }
    }

    fn get_opportunity_service(
        chain_id: ChainId,
    ) -> (MockService<ChainTypeSvm>, Vec<OpportunitySvm>) {
        let mut opportunity_service = MockService::<ChainTypeSvm>::default();
        let now = OffsetDateTime::now_utc();
        let router = Pubkey::new_unique();
        let user_wallet_address = Pubkey::new_unique();

        let user_token_address = Pubkey::new_unique();
        let searcher_token_address = Pubkey::new_unique();
        let amount = 100;

        let tokens_user_specified = QuoteTokens::UserTokenSpecified {
            user_token:     TokenAmountSvm {
                token: user_token_address,
                amount,
            },
            searcher_token: searcher_token_address,
        };
        let tokens_searcher_specified = QuoteTokens::SearcherTokenSpecified {
            searcher_token: TokenAmountSvm {
                token: searcher_token_address,
                amount,
            },
            user_token:     user_token_address,
        };
        let tokens_user_wsol = QuoteTokens::UserTokenSpecified {
            user_token:     TokenAmountSvm {
                token: spl_token::native_mint::id(),
                amount,
            },
            searcher_token: searcher_token_address,
        };
        let tokens_searcher_wsol = QuoteTokens::UserTokenSpecified {
            user_token:     TokenAmountSvm {
                token: user_token_address,
                amount,
            },
            searcher_token: spl_token::native_mint::id(),
        };
        let referral_fee_bps = 10;

        let fee_token = FeeToken::UserToken;
        let router_token_account = get_associated_token_address_with_program_id(
            &router,
            &user_token_address,
            &spl_token::id(),
        );
        let router_token_account_wsol = get_associated_token_address_with_program_id(
            &router,
            &spl_token::native_mint::id(),
            &spl_token::id(),
        );

        let permission_account_user_token_specified = get_quote_virtual_permission_account(
            &tokens_user_specified,
            &user_wallet_address,
            &router_token_account,
            referral_fee_bps,
        );
        let permission_account_searcher_token_specified = get_quote_virtual_permission_account(
            &tokens_searcher_specified,
            &user_wallet_address,
            &router_token_account,
            referral_fee_bps,
        );
        let permission_account_user_token_wsol = get_quote_virtual_permission_account(
            &tokens_user_wsol,
            &user_wallet_address,
            &router_token_account_wsol,
            referral_fee_bps,
        );
        let permission_account_searcher_token_wsol = get_quote_virtual_permission_account(
            &tokens_searcher_wsol,
            &user_wallet_address,
            &router_token_account,
            referral_fee_bps,
        );

        let opp_user_token_specified = OpportunitySvm {
            core_fields: OpportunityCoreFields::<TokenAmountSvm> {
                id:             Uuid::new_v4(),
                permission_key: OpportunitySvm::get_permission_key(
                    BidPaymentInstructionType::Swap,
                    router,
                    permission_account_user_token_specified,
                ),
                chain_id:       chain_id.clone(),
                sell_tokens:    vec![TokenAmountSvm {
                    token:  searcher_token_address,
                    amount: 0,
                }],
                buy_tokens:     vec![TokenAmountSvm {
                    token: user_token_address,
                    amount,
                }],
                creation_time:  now,
                refresh_time:   now,
            },
            router,
            permission_account: permission_account_user_token_specified,
            program: OpportunitySvmProgram::Swap(OpportunitySvmProgramSwap {
                user_wallet_address,
                platform_fee_bps: 0,
                token_program_user: spl_token::id(),
                token_program_searcher: spl_token::id(),
                fee_token: fee_token.clone(),
                referral_fee_bps,
                user_mint_user_balance: 0,
                token_account_initialization_config:
                    TokenAccountInitializationConfigs::searcher_payer(),
            }),
        };

        let opp_searcher_token_specified = OpportunitySvm {
            core_fields: OpportunityCoreFields::<TokenAmountSvm> {
                id:             Uuid::new_v4(),
                permission_key: OpportunitySvm::get_permission_key(
                    BidPaymentInstructionType::Swap,
                    router,
                    permission_account_searcher_token_specified,
                ),
                chain_id:       chain_id.clone(),
                sell_tokens:    vec![TokenAmountSvm {
                    token: searcher_token_address,
                    amount,
                }],
                buy_tokens:     vec![TokenAmountSvm {
                    token:  user_token_address,
                    amount: 0,
                }],
                creation_time:  now,
                refresh_time:   now,
            },
            router,
            permission_account: permission_account_searcher_token_specified,
            program: OpportunitySvmProgram::Swap(OpportunitySvmProgramSwap {
                user_wallet_address,
                platform_fee_bps: 0,
                token_program_user: spl_token::id(),
                token_program_searcher: spl_token::id(),
                fee_token: fee_token.clone(),
                referral_fee_bps,
                user_mint_user_balance: 0,
                token_account_initialization_config:
                    TokenAccountInitializationConfigs::searcher_payer(),
            }),
        };

        let opp_user_token_wsol = OpportunitySvm {
            core_fields: OpportunityCoreFields::<TokenAmountSvm> {
                id:             Uuid::new_v4(),
                permission_key: OpportunitySvm::get_permission_key(
                    BidPaymentInstructionType::Swap,
                    router,
                    permission_account_user_token_wsol,
                ),
                chain_id:       chain_id.clone(),
                sell_tokens:    vec![TokenAmountSvm {
                    token:  searcher_token_address,
                    amount: 0,
                }],
                buy_tokens:     vec![TokenAmountSvm {
                    token: spl_token::native_mint::id(),
                    amount,
                }],
                creation_time:  now,
                refresh_time:   now,
            },
            router,
            permission_account: permission_account_user_token_wsol,
            program: OpportunitySvmProgram::Swap(OpportunitySvmProgramSwap {
                user_wallet_address,
                platform_fee_bps: 0,
                token_program_user: spl_token::id(),
                token_program_searcher: spl_token::id(),
                fee_token: fee_token.clone(),
                referral_fee_bps,
                user_mint_user_balance: 0,
                token_account_initialization_config: TokenAccountInitializationConfigs {
                    user_ata_mint_user: TokenAccountInitializationConfig::SearcherPayer,
                    ..TokenAccountInitializationConfigs::searcher_payer()
                },
            }),
        };

        let opp_searcher_token_wsol = OpportunitySvm {
            core_fields: OpportunityCoreFields::<TokenAmountSvm> {
                id: Uuid::new_v4(),
                permission_key: OpportunitySvm::get_permission_key(
                    BidPaymentInstructionType::Swap,
                    router,
                    permission_account_searcher_token_wsol,
                ),
                chain_id,
                sell_tokens: vec![TokenAmountSvm {
                    token:  spl_token::native_mint::id(),
                    amount: 0,
                }],
                buy_tokens: vec![TokenAmountSvm {
                    token: user_token_address,
                    amount,
                }],
                creation_time: now,
                refresh_time: now,
            },
            router,
            permission_account: permission_account_searcher_token_wsol,
            program: OpportunitySvmProgram::Swap(OpportunitySvmProgramSwap {
                user_wallet_address,
                platform_fee_bps: 0,
                token_program_user: spl_token::id(),
                token_program_searcher: spl_token::id(),
                fee_token,
                referral_fee_bps,
                user_mint_user_balance: 0,
                token_account_initialization_config:
                    TokenAccountInitializationConfigs::searcher_payer(),
            }),
        };

        let opps = vec![
            opp_user_token_specified.clone(),
            opp_searcher_token_specified.clone(),
            opp_user_token_wsol.clone(),
            opp_searcher_token_wsol.clone(),
        ];
        let opps_cloned = opps.clone();

        opportunity_service
            .expect_get_live_opportunities()
            .returning(move |input| {
                opps.iter()
                    .filter(|opp| {
                        opp.chain_id == input.key.0 && opp.core_fields.permission_key == input.key.1
                    })
                    .cloned()
                    .collect()
            });
        opportunity_service
            .expect_get_live_opportunity_by_id()
            .returning(move |input| {
                opps_cloned
                    .iter()
                    .find(|opp| opp.core_fields.id == input.opportunity_id)
                    .cloned()
            });

        (
            opportunity_service,
            vec![
                opp_user_token_specified,
                opp_searcher_token_specified,
                opp_user_token_wsol,
                opp_searcher_token_wsol,
            ],
        )
    }

    fn get_service(mock_simulation: bool) -> (super::Service<Svm>, Vec<OpportunitySvm>) {
        let chain_id = "solana".to_string();
        let mut rpc_client = MockRpcClient::default();
        if mock_simulation {
            rpc_client.expect_send().returning(|_, _| {
                Ok(serde_json::json!({
                    "context": { "slot": 1 },
                    "value": {
                        "err": null,
                        "accounts": null,
                        "logs": [],
                        "returnData": {
                            "data": ["", "base64"],
                            "programId": "11111111111111111111111111111111",
                        },
                        "unitsConsumed": 0
                    }
                }))
            });
        }

        let broadcaster_client = MockRpcClient::default();
        let (opportunity_service, opportunities) = get_opportunity_service(chain_id.clone());
        let db = MockDatabase::<Svm>::default();
        let service = super::Service::new_with_mocks_svm(
            chain_id.clone(),
            db,
            opportunity_service,
            rpc_client,
            broadcaster_client,
        );

        (service, opportunities)
    }

    fn get_opportunity_params(
        opportunity: OpportunitySvm,
    ) -> opportunity_api::OpportunityParamsSvm {
        let api_opportunity: opportunity_api::Opportunity = opportunity.into();
        match api_opportunity {
            opportunity_api::Opportunity::Svm(opportunity_svm) => opportunity_svm.params,
            _ => panic!("Expected Svm opportunity"),
        }
    }

    struct SwapParams {
        user_wallet_address: Pubkey,
        router_account:      Pubkey,
        permission_account:  Pubkey,
    }

    fn get_opportunity_swap_params(opportunity: OpportunitySvm) -> SwapParams {
        let opportunity_params = get_opportunity_params(opportunity);
        let opportunity_api::OpportunityParamsSvm::V1(opportunity_params) = opportunity_params;
        match opportunity_params.program {
            opportunity_api::OpportunityParamsV1ProgramSvm::Swap {
                user_wallet_address,
                router_account,
                permission_account,
                ..
            } => SwapParams {
                user_wallet_address,
                router_account,
                permission_account,
            },
            _ => panic!("Expected swap program"),
        }
    }

    async fn get_verify_bid_result(
        service: Service<Svm>,
        searcher: Keypair,
        instructions: Vec<Instruction>,
        opportunity: OpportunitySvm,
    ) -> Result<VerificationResult<Svm>, RestError> {
        let mut transaction = Transaction::new_with_payer(&instructions, Some(&searcher.pubkey()));
        transaction.partial_sign(&[searcher], Hash::default());
        let bid_create = BidCreate::<Svm> {
            chain_id:        service.config.chain_id.clone(),
            initiation_time: OffsetDateTime::now_utc(),
            profile:         None,
            chain_data:      BidChainDataCreateSvm::Swap(BidChainDataSwapCreateSvm {
                opportunity_id: opportunity.core_fields.id,
                transaction:    transaction.into(),
            }),
        };
        service
            .verify_bid(super::VerifyBidInput { bid_create })
            .await
    }

    #[tokio::test]
    async fn test_verify_bid() {
        let (service, opportunities) = get_service(true);

        let bid_amount = 1;
        let searcher = Keypair::new();
        let instruction = svm::Svm::get_swap_instruction(GetSwapInstructionParams {
            searcher: searcher.pubkey(),
            opportunity_params: get_opportunity_params(opportunities[0].clone()),
            bid_amount,
            deadline: (OffsetDateTime::now_utc() + Duration::minutes(1)).unix_timestamp(),
            fee_receiver_relayer: Pubkey::new_unique(),
            relayer_signer: service.config.chain_config.express_relay.relayer.pubkey(),
        })
        .unwrap();
        let mut transaction = Transaction::new_with_payer(&[instruction], Some(&searcher.pubkey()));
        transaction.partial_sign(&[searcher], Hash::default());

        let bid_create = BidCreate::<Svm> {
            chain_id:        service.config.chain_id.clone(),
            initiation_time: OffsetDateTime::now_utc(),
            profile:         None,
            chain_data:      BidChainDataCreateSvm::Swap(BidChainDataSwapCreateSvm {
                opportunity_id: opportunities[0].core_fields.id,
                transaction:    transaction.clone().into(),
            }),
        };

        let result = service
            .verify_bid(super::VerifyBidInput { bid_create })
            .await
            .unwrap();
        let swap_params = get_opportunity_swap_params(opportunities[0].clone());
        assert_eq!(
            result.0,
            BidChainDataSvm {
                transaction:                  transaction.into(),
                permission_account:           swap_params.permission_account,
                router:                       swap_params.router_account,
                bid_payment_instruction_type: BidPaymentInstructionType::Swap,
            }
        );
        assert_eq!(result.1, bid_amount);
    }

    #[tokio::test]
    async fn test_verify_bid_when_multiple_compute_unit_price_instructions() {
        let (service, opportunities) = get_service(true);
        let searcher = Keypair::new();
        let instruction = compute_budget::ComputeBudgetInstruction::set_compute_unit_price(1);
        let result = get_verify_bid_result(
            service,
            searcher,
            vec![instruction.clone(), instruction],
            opportunities[0].clone(),
        )
        .await;
        assert_eq!(
            result.unwrap_err(),
            RestError::MultipleSetComputeUnitPriceInstructions,
        );
    }

    #[tokio::test]
    async fn test_verify_bid_when_no_compute_unit_price_instructions() {
        let (service, opportunities) = get_service(true);
        let searcher = Keypair::new();
        let minimum_budget = 10;
        service
            .repo
            .add_recent_priotization_fee(minimum_budget)
            .await;
        let result =
            get_verify_bid_result(service, searcher, vec![], opportunities[0].clone()).await;
        assert_eq!(
            result.unwrap_err(),
            RestError::SetComputeUnitPriceInstructionNotFound(minimum_budget),
        );
    }

    #[tokio::test]
    async fn test_verify_bid_when_compute_budget_is_low() {
        let (service, opportunities) = get_service(true);
        let searcher = Keypair::new();
        let minimum_budget = 10;
        let instruction =
            compute_budget::ComputeBudgetInstruction::set_compute_unit_price(minimum_budget - 1);
        service
            .repo
            .add_recent_priotization_fee(minimum_budget)
            .await;
        let result = get_verify_bid_result(
            service,
            searcher,
            vec![instruction],
            opportunities[0].clone(),
        )
        .await;
        assert_eq!(
            result.unwrap_err(),
            RestError::LowComputeUnitPrice(minimum_budget),
        );
    }

    #[tokio::test]
    async fn test_verify_bid_when_transaction_exceeds_size_limit() {
        let (service, opportunities) = get_service(true);
        let searcher = Keypair::new();
        let mut instructions = Vec::new();
        let swap_params = get_opportunity_swap_params(opportunities[0].clone());
        for _ in 0..61 {
            // Adjust number to exceed limit
            let transfer_instruction = system_instruction::transfer(
                &searcher.pubkey(),
                &swap_params.user_wallet_address,
                100,
            );
            instructions.push(transfer_instruction);
        }
        let result =
            get_verify_bid_result(service, searcher, instructions, opportunities[0].clone()).await;
        assert_eq!(
            result.unwrap_err(),
            RestError::TransactionSizeTooLarge(1235, PACKET_DATA_SIZE)
        );
    }

    #[tokio::test]
    async fn test_verify_bid_when_opportunity_not_found() {
        let (service, opportunities) = get_service(true);
        let searcher = Keypair::new();
        let mut opportunity = opportunities[0].clone();
        opportunity.core_fields.id = Uuid::new_v4();
        let result = get_verify_bid_result(service, searcher, vec![], opportunity).await;
        assert_eq!(result.unwrap_err(), RestError::SwapOpportunityNotFound);
    }

    #[tokio::test]
    async fn test_verify_bid_when_unsupported_system_program_instruction() {
        let (service, opportunities) = get_service(true);
        let instructions = vec![
            system_instruction::advance_nonce_account(&Pubkey::new_unique(), &Pubkey::new_unique()),
            system_instruction::create_account(
                &Pubkey::new_unique(),
                &Pubkey::new_unique(),
                0,
                0,
                &Pubkey::new_unique(),
            ),
            system_instruction::allocate(&Pubkey::new_unique(), 0),
            system_instruction::assign(&Pubkey::new_unique(), &Pubkey::new_unique()),
            system_instruction::create_account_with_seed(
                &Pubkey::new_unique(),
                &Pubkey::new_unique(),
                &Pubkey::new_unique(),
                "",
                0,
                0,
                &Pubkey::new_unique(),
            ),
        ];
        for instruction in instructions.into_iter() {
            let searcher = Keypair::new();
            let result = get_verify_bid_result(
                service.clone(),
                searcher,
                vec![instruction],
                opportunities[0].clone(),
            )
            .await;
            assert_eq!(
                result.unwrap_err(),
                RestError::InvalidInstruction(
                    Some(0),
                    InstructionError::UnsupportedSystemProgramInstruction
                )
            );
        }
    }

    #[tokio::test]
    async fn test_verify_bid_when_unsupported_token_instruction() {
        let (service, opportunities) = get_service(true);
        let instructions = vec![
            spl_token::instruction::initialize_account(
                &spl_token::id(),
                &Pubkey::new_unique(),
                &Pubkey::new_unique(),
                &Pubkey::new_unique(),
            )
            .unwrap(),
            spl_token::instruction::initialize_account2(
                &spl_token::id(),
                &Pubkey::new_unique(),
                &Pubkey::new_unique(),
                &Pubkey::new_unique(),
            )
            .unwrap(),
            spl_token::instruction::initialize_account3(
                &spl_token::id(),
                &Pubkey::new_unique(),
                &Pubkey::new_unique(),
                &Pubkey::new_unique(),
            )
            .unwrap(),
            spl_token::instruction::initialize_mint(
                &spl_token::id(),
                &Pubkey::new_unique(),
                &Pubkey::new_unique(),
                None,
                0,
            )
            .unwrap(),
            spl_token::instruction::initialize_mint2(
                &spl_token::id(),
                &Pubkey::new_unique(),
                &Pubkey::new_unique(),
                None,
                0,
            )
            .unwrap(),
            spl_token::instruction::transfer(
                &spl_token::id(),
                &Pubkey::new_unique(),
                &Pubkey::new_unique(),
                &Pubkey::new_unique(),
                &[],
                0,
            )
            .unwrap(),
            spl_token::instruction::approve(
                &spl_token::id(),
                &Pubkey::new_unique(),
                &Pubkey::new_unique(),
                &Pubkey::new_unique(),
                &[],
                0,
            )
            .unwrap(),
            spl_token::instruction::revoke(
                &spl_token::id(),
                &Pubkey::new_unique(),
                &Pubkey::new_unique(),
                &[],
            )
            .unwrap(),
            spl_token::instruction::set_authority(
                &spl_token::id(),
                &Pubkey::new_unique(),
                None,
                spl_token::instruction::AuthorityType::AccountOwner,
                &Pubkey::new_unique(),
                &[],
            )
            .unwrap(),
            spl_token::instruction::mint_to(
                &spl_token::id(),
                &Pubkey::new_unique(),
                &Pubkey::new_unique(),
                &Pubkey::new_unique(),
                &[],
                0,
            )
            .unwrap(),
            spl_token::instruction::burn(
                &spl_token::id(),
                &Pubkey::new_unique(),
                &Pubkey::new_unique(),
                &Pubkey::new_unique(),
                &[],
                0,
            )
            .unwrap(),
        ];
        for instruction in instructions.into_iter() {
            let data = instruction.data.clone();
            let ix_parsed = TokenInstruction::unpack(&data).unwrap();
            let searcher = Keypair::new();
            let result = get_verify_bid_result(
                service.clone(),
                searcher,
                vec![instruction],
                opportunities[0].clone(),
            )
            .await;
            assert_eq!(
                result.unwrap_err(),
                RestError::InvalidInstruction(
                    Some(0),
                    InstructionError::UnsupportedSplTokenInstruction(format!("{:?}", ix_parsed)),
                )
            );
        }
    }

    #[tokio::test]
    async fn test_verify_bid_when_unsupported_associated_token_account_instruction() {
        let (service, opportunities) = get_service(true);
        let instructions = vec![recover_nested(
            &Pubkey::new_unique(),
            &Pubkey::new_unique(),
            &Pubkey::new_unique(),
            &spl_token::id(),
        )];
        for instruction in instructions.into_iter() {
            let data = instruction.data.clone();
            let ix_parsed = AssociatedTokenAccountInstruction::try_from_slice(&data)
                .map_err(|e| {
                    InstructionError::InvalidAssociatedTokenAccountInstruction(e.to_string())
                })
                .unwrap();
            let searcher = Keypair::new();
            let result = get_verify_bid_result(
                service.clone(),
                searcher,
                vec![instruction],
                opportunities[0].clone(),
            )
            .await;
            assert_eq!(
                result.unwrap_err(),
                RestError::InvalidInstruction(
                    Some(0),
                    InstructionError::UnsupportedAssociatedTokenAccountInstruction(ix_parsed),
                )
            );
        }
    }

    #[tokio::test]
    async fn test_verify_bid_when_unsupported_program() {
        let (service, opportunities) = get_service(true);
        let program_id = Pubkey::new_unique();
        let instructions = vec![Instruction::new_with_bincode(program_id, &"", vec![])];
        for instruction in instructions.into_iter() {
            let searcher = Keypair::new();
            let result = get_verify_bid_result(
                service.clone(),
                searcher,
                vec![instruction],
                opportunities[0].clone(),
            )
            .await;
            assert_eq!(
                result.unwrap_err(),
                RestError::InvalidInstruction(
                    Some(0),
                    InstructionError::UnsupportedProgram(program_id)
                )
            );
        }
    }

    #[tokio::test]
    async fn test_verify_bid_when_multiple_express_relay_instructions() {
        let (service, opportunities) = get_service(true);
        let searcher = Keypair::new();
        let swap_instruction = svm::Svm::get_swap_instruction(GetSwapInstructionParams {
            searcher:             searcher.pubkey(),
            opportunity_params:   get_opportunity_params(opportunities[0].clone()),
            bid_amount:           1,
            deadline:             (OffsetDateTime::now_utc() + Duration::minutes(1))
                .unix_timestamp(),
            fee_receiver_relayer: Pubkey::new_unique(),
            relayer_signer:       service.config.chain_config.express_relay.relayer.pubkey(),
        })
        .unwrap();
        let submit_bid_instruction =
            svm::Svm::get_submit_bid_instruction(GetSubmitBidInstructionParams {
                chain_id:             service.config.chain_id.clone(),
                amount:               1,
                deadline:             (OffsetDateTime::now_utc() + Duration::minutes(1))
                    .unix_timestamp(),
                searcher:             searcher.pubkey(),
                permission:           Pubkey::new_unique(),
                router:               Pubkey::new_unique(),
                relayer_signer:       service.config.chain_config.express_relay.relayer.pubkey(),
                fee_receiver_relayer: Pubkey::new_unique(),
            })
            .unwrap();
        let result = get_verify_bid_result(
            service,
            searcher,
            vec![swap_instruction, submit_bid_instruction],
            opportunities[0].clone(),
        )
        .await;
        assert_eq!(
            result.unwrap_err(),
            RestError::InvalidExpressRelayInstructionCount(2),
        );
    }

    #[tokio::test]
    async fn test_verify_bid_when_no_express_relay_instructions() {
        let (service, opportunities) = get_service(true);
        let searcher = Keypair::new();
        let result =
            get_verify_bid_result(service, searcher, vec![], opportunities[0].clone()).await;
        assert_eq!(
            result.unwrap_err(),
            RestError::InvalidExpressRelayInstructionCount(0),
        );
    }

    #[tokio::test]
    async fn test_verify_bid_when_invalid_user_wallet_address() {
        let (service, opportunities) = get_service(true);
        let searcher = Keypair::new();
        let mut opportunity = opportunities[0].clone();
        let mut program = match opportunity.program {
            OpportunitySvmProgram::Swap(program) => program,
            _ => panic!("Expected swap program"),
        };
        let expected = program.user_wallet_address;
        program.user_wallet_address = Pubkey::new_unique();
        let found = program.user_wallet_address;
        opportunity.program = OpportunitySvmProgram::Swap(program);
        let swap_instruction = svm::Svm::get_swap_instruction(GetSwapInstructionParams {
            searcher:             searcher.pubkey(),
            opportunity_params:   get_opportunity_params(opportunity),
            bid_amount:           1,
            deadline:             (OffsetDateTime::now_utc() + Duration::minutes(1))
                .unix_timestamp(),
            fee_receiver_relayer: Pubkey::new_unique(),
            relayer_signer:       service.config.chain_config.express_relay.relayer.pubkey(),
        })
        .unwrap();
        let result = get_verify_bid_result(
            service,
            searcher,
            vec![swap_instruction],
            opportunities[0].clone(),
        )
        .await;
        assert_eq!(
            result.unwrap_err(),
            RestError::InvalidSwapInstruction(SwapInstructionError::UserWalletAddress {
                expected,
                found
            })
        );
    }

    #[tokio::test]
    async fn test_verify_bid_when_invalid_mint_searcher() {
        let (service, opportunities) = get_service(true);
        let searcher = Keypair::new();
        let mut opportunity = opportunities[0].clone();
        let expected = opportunity.core_fields.sell_tokens[0].token;
        opportunity.core_fields.sell_tokens[0].token = Pubkey::new_unique();
        let found = opportunity.core_fields.sell_tokens[0].token;
        let swap_instruction = svm::Svm::get_swap_instruction(GetSwapInstructionParams {
            searcher:             searcher.pubkey(),
            opportunity_params:   get_opportunity_params(opportunity),
            bid_amount:           1,
            deadline:             (OffsetDateTime::now_utc() + Duration::minutes(1))
                .unix_timestamp(),
            fee_receiver_relayer: Pubkey::new_unique(),
            relayer_signer:       service.config.chain_config.express_relay.relayer.pubkey(),
        })
        .unwrap();
        let result = get_verify_bid_result(
            service,
            searcher,
            vec![swap_instruction],
            opportunities[0].clone(),
        )
        .await;
        assert_eq!(
            result.unwrap_err(),
            RestError::InvalidSwapInstruction(SwapInstructionError::MintSearcher {
                expected,
                found
            })
        );
    }

    #[tokio::test]
    async fn test_verify_bid_when_invalid_mint_user() {
        let (service, opportunities) = get_service(true);
        let searcher = Keypair::new();
        let mut opportunity = opportunities[0].clone();
        let expected = opportunity.core_fields.buy_tokens[0].token;
        opportunity.core_fields.buy_tokens[0].token = Pubkey::new_unique();
        let found = opportunity.core_fields.buy_tokens[0].token;
        let swap_instruction = svm::Svm::get_swap_instruction(GetSwapInstructionParams {
            searcher:             searcher.pubkey(),
            opportunity_params:   get_opportunity_params(opportunity),
            bid_amount:           1,
            deadline:             (OffsetDateTime::now_utc() + Duration::minutes(1))
                .unix_timestamp(),
            fee_receiver_relayer: Pubkey::new_unique(),
            relayer_signer:       service.config.chain_config.express_relay.relayer.pubkey(),
        })
        .unwrap();
        let result = get_verify_bid_result(
            service,
            searcher,
            vec![swap_instruction],
            opportunities[0].clone(),
        )
        .await;
        assert_eq!(
            result.unwrap_err(),
            RestError::InvalidSwapInstruction(SwapInstructionError::MintUser { expected, found })
        );
    }

    #[tokio::test]
    async fn test_verify_bid_when_invalid_token_program_searcher() {
        let (service, opportunities) = get_service(true);
        let searcher = Keypair::new();
        let mut opportunity = opportunities[0].clone();
        let mut program = match opportunity.program {
            OpportunitySvmProgram::Swap(program) => program,
            _ => panic!("Expected swap program"),
        };
        let expected = program.token_program_searcher;
        program.token_program_searcher = Pubkey::new_unique();
        let found = program.token_program_searcher;
        opportunity.program = OpportunitySvmProgram::Swap(program);
        let swap_instruction = svm::Svm::get_swap_instruction(GetSwapInstructionParams {
            searcher:             searcher.pubkey(),
            opportunity_params:   get_opportunity_params(opportunity),
            bid_amount:           1,
            deadline:             (OffsetDateTime::now_utc() + Duration::minutes(1))
                .unix_timestamp(),
            fee_receiver_relayer: Pubkey::new_unique(),
            relayer_signer:       service.config.chain_config.express_relay.relayer.pubkey(),
        })
        .unwrap();
        let result = get_verify_bid_result(
            service,
            searcher,
            vec![swap_instruction],
            opportunities[0].clone(),
        )
        .await;
        assert_eq!(
            result.unwrap_err(),
            RestError::InvalidSwapInstruction(SwapInstructionError::TokenProgramSearcher {
                expected,
                found
            })
        );
    }

    #[tokio::test]
    async fn test_verify_bid_when_invalid_token_program_user() {
        let (service, opportunities) = get_service(true);
        let searcher = Keypair::new();
        let mut opportunity = opportunities[0].clone();
        let mut program = match opportunity.program {
            OpportunitySvmProgram::Swap(program) => program,
            _ => panic!("Expected swap program"),
        };
        let expected = program.token_program_user;
        program.token_program_user = Pubkey::new_unique();
        let found = program.token_program_user;
        opportunity.program = OpportunitySvmProgram::Swap(program);
        let swap_instruction = svm::Svm::get_swap_instruction(GetSwapInstructionParams {
            searcher:             searcher.pubkey(),
            opportunity_params:   get_opportunity_params(opportunity),
            bid_amount:           1,
            deadline:             (OffsetDateTime::now_utc() + Duration::minutes(1))
                .unix_timestamp(),
            fee_receiver_relayer: Pubkey::new_unique(),
            relayer_signer:       service.config.chain_config.express_relay.relayer.pubkey(),
        })
        .unwrap();
        let result = get_verify_bid_result(
            service,
            searcher,
            vec![swap_instruction],
            opportunities[0].clone(),
        )
        .await;
        assert_eq!(
            result.unwrap_err(),
            RestError::InvalidSwapInstruction(SwapInstructionError::TokenProgramUser {
                expected,
                found
            })
        );
    }

    #[tokio::test]
    async fn test_verify_bid_when_invalid_token_amount_searcher() {
        let (service, opportunities) = get_service(true);
        let searcher = Keypair::new();
        let mut opportunity = opportunities[1].clone();
        let mut token = opportunity.core_fields.sell_tokens[0].clone();
        token.amount += 1;
        opportunity.core_fields.sell_tokens[0] = token.clone();
        let swap_instruction = svm::Svm::get_swap_instruction(GetSwapInstructionParams {
            searcher:             searcher.pubkey(),
            opportunity_params:   get_opportunity_params(opportunity),
            bid_amount:           1,
            deadline:             (OffsetDateTime::now_utc() + Duration::minutes(1))
                .unix_timestamp(),
            fee_receiver_relayer: Pubkey::new_unique(),
            relayer_signer:       service.config.chain_config.express_relay.relayer.pubkey(),
        })
        .unwrap();
        let result = get_verify_bid_result(
            service,
            searcher,
            vec![swap_instruction],
            opportunities[1].clone(),
        )
        .await;
        assert_eq!(
            result.unwrap_err(),
            RestError::InvalidSwapInstruction(SwapInstructionError::AmountSearcher {
                expected: token.amount - 1,
                found:    token.amount,
            })
        );
    }

    #[tokio::test]
    async fn test_verify_bid_when_invalid_token_amount_user() {
        let (service, opportunities) = get_service(true);
        let searcher = Keypair::new();
        let mut opportunity = opportunities[0].clone();
        let mut token = opportunity.core_fields.buy_tokens[0].clone();
        token.amount += 1;
        opportunity.core_fields.buy_tokens[0] = token.clone();
        let swap_instruction = svm::Svm::get_swap_instruction(GetSwapInstructionParams {
            searcher:             searcher.pubkey(),
            opportunity_params:   get_opportunity_params(opportunity),
            bid_amount:           1,
            deadline:             (OffsetDateTime::now_utc() + Duration::minutes(1))
                .unix_timestamp(),
            fee_receiver_relayer: Pubkey::new_unique(),
            relayer_signer:       service.config.chain_config.express_relay.relayer.pubkey(),
        })
        .unwrap();
        let result = get_verify_bid_result(
            service,
            searcher,
            vec![swap_instruction],
            opportunities[0].clone(),
        )
        .await;
        assert_eq!(
            result.unwrap_err(),
            RestError::InvalidSwapInstruction(SwapInstructionError::AmountUser {
                expected: token.amount - 1,
                found:    token.amount,
            })
        );
    }

    #[tokio::test]
    async fn test_verify_bid_when_invalid_fee_token() {
        let (service, opportunities) = get_service(true);
        let searcher = Keypair::new();
        let mut opportunity = opportunities[0].clone();
        let mut program = match opportunity.program {
            OpportunitySvmProgram::Swap(program) => program,
            _ => panic!("Expected swap program"),
        };
        program.fee_token = FeeToken::SearcherToken;
        opportunity.program = OpportunitySvmProgram::Swap(program);
        let swap_instruction = svm::Svm::get_swap_instruction(GetSwapInstructionParams {
            searcher:             searcher.pubkey(),
            opportunity_params:   get_opportunity_params(opportunity),
            bid_amount:           1,
            deadline:             (OffsetDateTime::now_utc() + Duration::minutes(1))
                .unix_timestamp(),
            fee_receiver_relayer: Pubkey::new_unique(),
            relayer_signer:       service.config.chain_config.express_relay.relayer.pubkey(),
        })
        .unwrap();
        let result = get_verify_bid_result(
            service,
            searcher,
            vec![swap_instruction],
            opportunities[0].clone(),
        )
        .await;
        assert_eq!(
            result.unwrap_err(),
            RestError::InvalidSwapInstruction(SwapInstructionError::FeeToken {
                expected: FeeToken::UserToken,
                found:    express_relay::FeeToken::Searcher,
            })
        );
    }

    #[tokio::test]
    async fn test_verify_bid_when_invalid_referral_fee_bps() {
        let (service, opportunities) = get_service(true);
        let searcher = Keypair::new();
        let mut opportunity = opportunities[0].clone();
        let mut program = match opportunity.program {
            OpportunitySvmProgram::Swap(program) => program,
            _ => panic!("Expected swap program"),
        };
        program.referral_fee_bps += 1;
        opportunity.program = OpportunitySvmProgram::Swap(program.clone());
        let swap_instruction = svm::Svm::get_swap_instruction(GetSwapInstructionParams {
            searcher:             searcher.pubkey(),
            opportunity_params:   get_opportunity_params(opportunity),
            bid_amount:           1,
            deadline:             (OffsetDateTime::now_utc() + Duration::minutes(1))
                .unix_timestamp(),
            fee_receiver_relayer: Pubkey::new_unique(),
            relayer_signer:       service.config.chain_config.express_relay.relayer.pubkey(),
        })
        .unwrap();
        let result = get_verify_bid_result(
            service,
            searcher,
            vec![swap_instruction],
            opportunities[0].clone(),
        )
        .await;
        assert_eq!(
            result.unwrap_err(),
            RestError::InvalidSwapInstruction(SwapInstructionError::ReferralFee {
                expected: program.referral_fee_bps - 1,
                found:    program.referral_fee_bps,
            })
        );
    }

    #[tokio::test]
    async fn test_verify_bid_when_no_transfer_instruction() {
        let (service, opportunities) = get_service(false);
        let searcher = Keypair::new();
        let swap_instruction = svm::Svm::get_swap_instruction(GetSwapInstructionParams {
            searcher:             searcher.pubkey(),
            opportunity_params:   get_opportunity_params(opportunities[2].clone()),
            bid_amount:           1,
            deadline:             (OffsetDateTime::now_utc() + Duration::minutes(1))
                .unix_timestamp(),
            fee_receiver_relayer: Pubkey::new_unique(),
            relayer_signer:       service.config.chain_config.express_relay.relayer.pubkey(),
        })
        .unwrap();
        let result = get_verify_bid_result(
            service,
            searcher,
            vec![swap_instruction],
            opportunities[2].clone(), // User token wsol
        )
        .await;
        assert_eq!(
            result.unwrap_err(),
            RestError::InvalidInstruction(None, InstructionError::InvalidTransferInstructionsCount)
        );
    }

    #[tokio::test]
    async fn test_verify_bid_when_no_transfer_instruction_is_allowed() {
        let (service, opportunities) = get_service(false);
        let searcher = Keypair::new();
        let swap_instruction = svm::Svm::get_swap_instruction(GetSwapInstructionParams {
            searcher:             searcher.pubkey(),
            opportunity_params:   get_opportunity_params(opportunities[0].clone()),
            bid_amount:           1,
            deadline:             (OffsetDateTime::now_utc() + Duration::minutes(1))
                .unix_timestamp(),
            fee_receiver_relayer: Pubkey::new_unique(),
            relayer_signer:       service.config.chain_config.express_relay.relayer.pubkey(),
        })
        .unwrap();
        let transfer_instruction =
            system_instruction::transfer(&searcher.pubkey(), &Pubkey::new_unique(), 1);
        let result = get_verify_bid_result(
            service,
            searcher,
            vec![swap_instruction, transfer_instruction],
            opportunities[0].clone(),
        )
        .await;
        assert_eq!(
            result.unwrap_err(),
            RestError::InvalidInstruction(None, InstructionError::TransferInstructionNotAllowed)
        );
    }

    #[tokio::test]
    async fn test_verify_bid_when_no_close_account_instruction_is_allowed() {
        let (service, opportunities) = get_service(false);
        let searcher = Keypair::new();
        let swap_instruction = svm::Svm::get_swap_instruction(GetSwapInstructionParams {
            searcher:             searcher.pubkey(),
            opportunity_params:   get_opportunity_params(opportunities[0].clone()),
            bid_amount:           1,
            deadline:             (OffsetDateTime::now_utc() + Duration::minutes(1))
                .unix_timestamp(),
            fee_receiver_relayer: Pubkey::new_unique(),
            relayer_signer:       service.config.chain_config.express_relay.relayer.pubkey(),
        })
        .unwrap();
        let close_account_instruction = spl_token::instruction::close_account(
            &spl_token::id(),
            &searcher.pubkey(),
            &searcher.pubkey(),
            &searcher.pubkey(),
            &[],
        )
        .unwrap();
        let result = get_verify_bid_result(
            service,
            searcher,
            vec![swap_instruction, close_account_instruction],
            opportunities[0].clone(),
        )
        .await;
        assert_eq!(
            result.unwrap_err(),
            RestError::InvalidInstruction(
                None,
                InstructionError::CloseAccountInstructionNotAllowed
            )
        );
    }

    #[tokio::test]
    async fn test_verify_bid_when_multiple_transfer_instructions() {
        let (service, opportunities) = get_service(false);
        let searcher = Keypair::new();
        let opportunity = opportunities[2].clone(); // User token wsol
        let swap_instruction = svm::Svm::get_swap_instruction(GetSwapInstructionParams {
            searcher:             searcher.pubkey(),
            opportunity_params:   get_opportunity_params(opportunity.clone()),
            bid_amount:           1,
            deadline:             (OffsetDateTime::now_utc() + Duration::minutes(1))
                .unix_timestamp(),
            fee_receiver_relayer: Pubkey::new_unique(),
            relayer_signer:       service.config.chain_config.express_relay.relayer.pubkey(),
        })
        .unwrap();
        let transfer_instruction =
            system_instruction::transfer(&searcher.pubkey(), &Pubkey::new_unique(), 1);
        let result = get_verify_bid_result(
            service,
            searcher,
            vec![
                swap_instruction,
                transfer_instruction.clone(),
                transfer_instruction,
            ],
            opportunity,
        )
        .await;
        assert_eq!(
            result.unwrap_err(),
            RestError::InvalidInstruction(None, InstructionError::InvalidTransferInstructionsCount)
        );
    }

    #[tokio::test]
    async fn test_verify_bid_when_invalid_from_account_transfer_instruction() {
        let (service, opportunities) = get_service(false);
        let searcher = Keypair::new();
        let opportunity = opportunities[2].clone(); // User token wsol
        let swap_instruction = svm::Svm::get_swap_instruction(GetSwapInstructionParams {
            searcher:             searcher.pubkey(),
            opportunity_params:   get_opportunity_params(opportunity.clone()),
            bid_amount:           1,
            deadline:             (OffsetDateTime::now_utc() + Duration::minutes(1))
                .unix_timestamp(),
            fee_receiver_relayer: Pubkey::new_unique(),
            relayer_signer:       service.config.chain_config.express_relay.relayer.pubkey(),
        })
        .unwrap();
        let program = match opportunity.program.clone() {
            OpportunitySvmProgram::Swap(program) => program,
            _ => panic!("Expected swap program"),
        };
        let expected = program.user_wallet_address;
        let found = Pubkey::new_unique();
        let transfer_instruction = system_instruction::transfer(&found, &Pubkey::new_unique(), 1);
        let result = get_verify_bid_result(
            service,
            searcher,
            vec![swap_instruction, transfer_instruction],
            opportunity,
        )
        .await;
        assert_eq!(
            result.unwrap_err(),
            RestError::InvalidInstruction(
                None,
                InstructionError::InvalidFromAccountTransferInstruction { expected, found }
            )
        );
    }

    #[tokio::test]
    async fn test_verify_bid_when_invalid_to_account_transfer_instruction() {
        let (service, opportunities) = get_service(false);
        let searcher = Keypair::new();
        let opportunity = opportunities[2].clone(); // User token wsol
        let swap_instruction = svm::Svm::get_swap_instruction(GetSwapInstructionParams {
            searcher:             searcher.pubkey(),
            opportunity_params:   get_opportunity_params(opportunity.clone()),
            bid_amount:           1,
            deadline:             (OffsetDateTime::now_utc() + Duration::minutes(1))
                .unix_timestamp(),
            fee_receiver_relayer: Pubkey::new_unique(),
            relayer_signer:       service.config.chain_config.express_relay.relayer.pubkey(),
        })
        .unwrap();
        let program = match opportunity.program.clone() {
            OpportunitySvmProgram::Swap(program) => program,
            _ => panic!("Expected swap program"),
        };
        let expected = get_associated_token_address(
            &program.user_wallet_address,
            &spl_token::native_mint::id(),
        );
        let found = Pubkey::new_unique();
        let transfer_instruction =
            system_instruction::transfer(&program.user_wallet_address, &found, 1);
        let result = get_verify_bid_result(
            service,
            searcher,
            vec![swap_instruction, transfer_instruction],
            opportunity,
        )
        .await;
        assert_eq!(
            result.unwrap_err(),
            RestError::InvalidInstruction(
                None,
                InstructionError::InvalidToAccountTransferInstruction { expected, found }
            )
        );
    }

    #[tokio::test]
    async fn test_verify_bid_when_invalid_amount_transfer_instruction() {
        let (service, opportunities) = get_service(false);
        let searcher = Keypair::new();
        let opportunity = opportunities[2].clone(); // User token wsol
        let swap_instruction = svm::Svm::get_swap_instruction(GetSwapInstructionParams {
            searcher:             searcher.pubkey(),
            opportunity_params:   get_opportunity_params(opportunity.clone()),
            bid_amount:           1,
            deadline:             (OffsetDateTime::now_utc() + Duration::minutes(1))
                .unix_timestamp(),
            fee_receiver_relayer: Pubkey::new_unique(),
            relayer_signer:       service.config.chain_config.express_relay.relayer.pubkey(),
        })
        .unwrap();
        let program = match opportunity.program.clone() {
            OpportunitySvmProgram::Swap(program) => program,
            _ => panic!("Expected swap program"),
        };
        let expected = opportunity.buy_tokens[0].amount;
        let found = opportunity.buy_tokens[0].amount + 1;
        let transfer_instruction = system_instruction::transfer(
            &program.user_wallet_address,
            &get_associated_token_address(
                &program.user_wallet_address,
                &spl_token::native_mint::id(),
            ),
            found,
        );
        let result = get_verify_bid_result(
            service,
            searcher,
            vec![swap_instruction, transfer_instruction],
            opportunity,
        )
        .await;
        assert_eq!(
            result.unwrap_err(),
            RestError::InvalidInstruction(
                None,
                InstructionError::InvalidAmountTransferInstruction { expected, found }
            )
        );
    }

    #[tokio::test]
    async fn test_verify_bid_when_multiple_sync_native_instructions() {
        let (service, opportunities) = get_service(false);
        let searcher = Keypair::new();
        let opportunity = opportunities[2].clone(); // User token wsol
        let swap_instruction = svm::Svm::get_swap_instruction(GetSwapInstructionParams {
            searcher:             searcher.pubkey(),
            opportunity_params:   get_opportunity_params(opportunity.clone()),
            bid_amount:           1,
            deadline:             (OffsetDateTime::now_utc() + Duration::minutes(1))
                .unix_timestamp(),
            fee_receiver_relayer: Pubkey::new_unique(),
            relayer_signer:       service.config.chain_config.express_relay.relayer.pubkey(),
        })
        .unwrap();
        let program = match opportunity.program.clone() {
            OpportunitySvmProgram::Swap(program) => program,
            _ => panic!("Expected swap program"),
        };
        let ata = &get_associated_token_address(
            &program.user_wallet_address,
            &spl_token::native_mint::id(),
        );
        let transfer_instruction = system_instruction::transfer(
            &program.user_wallet_address,
            &get_associated_token_address(
                &program.user_wallet_address,
                &spl_token::native_mint::id(),
            ),
            opportunity.buy_tokens[0].amount,
        );
        let sync_native_instruction =
            spl_token::instruction::sync_native(&spl_token::id(), ata).unwrap();
        let result = get_verify_bid_result(
            service,
            searcher,
            vec![
                swap_instruction,
                transfer_instruction,
                sync_native_instruction.clone(),
                sync_native_instruction,
            ],
            opportunity,
        )
        .await;
        assert_eq!(
            result.unwrap_err(),
            RestError::InvalidInstruction(
                None,
                InstructionError::InvalidSyncNativeInstructionCount(*ata)
            )
        );
    }

    #[tokio::test]
    async fn test_verify_bid_when_no_sync_native_instructions() {
        let (service, opportunities) = get_service(false);
        let searcher = Keypair::new();
        let opportunity = opportunities[2].clone(); // User token wsol
        let swap_instruction = svm::Svm::get_swap_instruction(GetSwapInstructionParams {
            searcher:             searcher.pubkey(),
            opportunity_params:   get_opportunity_params(opportunity.clone()),
            bid_amount:           1,
            deadline:             (OffsetDateTime::now_utc() + Duration::minutes(1))
                .unix_timestamp(),
            fee_receiver_relayer: Pubkey::new_unique(),
            relayer_signer:       service.config.chain_config.express_relay.relayer.pubkey(),
        })
        .unwrap();
        let program = match opportunity.program.clone() {
            OpportunitySvmProgram::Swap(program) => program,
            _ => panic!("Expected swap program"),
        };
        let ata = &get_associated_token_address(
            &program.user_wallet_address,
            &spl_token::native_mint::id(),
        );
        let transfer_instruction = system_instruction::transfer(
            &program.user_wallet_address,
            &get_associated_token_address(
                &program.user_wallet_address,
                &spl_token::native_mint::id(),
            ),
            opportunity.buy_tokens[0].amount,
        );
        let result = get_verify_bid_result(
            service,
            searcher,
            vec![swap_instruction, transfer_instruction],
            opportunity,
        )
        .await;
        assert_eq!(
            result.unwrap_err(),
            RestError::InvalidInstruction(
                None,
                InstructionError::InvalidSyncNativeInstructionCount(*ata)
            )
        );
    }

    #[tokio::test]
    async fn test_verify_bid_user_wsol() {
        let (service, opportunities) = get_service(true);
        let opportunity = opportunities[2].clone(); // User token wsol
        let bid_amount = 1;
        let searcher = Keypair::new();
        let swap_instruction = svm::Svm::get_swap_instruction(GetSwapInstructionParams {
            searcher: searcher.pubkey(),
            opportunity_params: get_opportunity_params(opportunity.clone()),
            bid_amount,
            deadline: (OffsetDateTime::now_utc() + Duration::minutes(1)).unix_timestamp(),
            fee_receiver_relayer: Pubkey::new_unique(),
            relayer_signer: service.config.chain_config.express_relay.relayer.pubkey(),
        })
        .unwrap();
        let program = match opportunity.program.clone() {
            OpportunitySvmProgram::Swap(program) => program,
            _ => panic!("Expected swap program"),
        };
        let ata = &get_associated_token_address(
            &program.user_wallet_address,
            &spl_token::native_mint::id(),
        );
        let transfer_instruction = system_instruction::transfer(
            &program.user_wallet_address,
            &get_associated_token_address(
                &program.user_wallet_address,
                &spl_token::native_mint::id(),
            ),
            opportunity.buy_tokens[0].amount,
        );
        let sync_native_instruction =
            spl_token::instruction::sync_native(&spl_token::id(), ata).unwrap();
        let user_close_account_instruction = spl_token::instruction::close_account(
            &spl_token::id(),
            &get_associated_token_address(
                &program.user_wallet_address,
                &spl_token::native_mint::id(),
            ),
            &program.user_wallet_address,
            &program.user_wallet_address,
            &[],
        )
        .unwrap();
        let mut transaction = Transaction::new_with_payer(
            &[
                transfer_instruction,
                sync_native_instruction,
                swap_instruction,
                user_close_account_instruction,
            ],
            Some(&searcher.pubkey()),
        );
        transaction.partial_sign(&[searcher], Hash::default());
        let bid_create = BidCreate::<Svm> {
            chain_id:        service.config.chain_id.clone(),
            initiation_time: OffsetDateTime::now_utc(),
            profile:         None,
            chain_data:      BidChainDataCreateSvm::Swap(BidChainDataSwapCreateSvm {
                opportunity_id: opportunity.core_fields.id,
                transaction:    transaction.clone().into(),
            }),
        };

        let result = service
            .verify_bid(super::VerifyBidInput { bid_create })
            .await
            .unwrap();
        let swap_params = get_opportunity_swap_params(opportunity);
        assert_eq!(
            result.0,
            BidChainDataSvm {
                transaction:                  transaction.into(),
                permission_account:           swap_params.permission_account,
                router:                       swap_params.router_account,
                bid_payment_instruction_type: BidPaymentInstructionType::Swap,
            }
        );
        assert_eq!(result.1, bid_amount);
    }

    #[tokio::test]
    async fn test_verify_bid_user_wsol_when_not_closing_user_wsol_account() {
        let (service, opportunities) = get_service(true);
        let opportunity = opportunities[2].clone(); // User token wsol
        let bid_amount = 1;
        let searcher = Keypair::new();
        let swap_instruction = svm::Svm::get_swap_instruction(GetSwapInstructionParams {
            searcher: searcher.pubkey(),
            opportunity_params: get_opportunity_params(opportunity.clone()),
            bid_amount,
            deadline: (OffsetDateTime::now_utc() + Duration::minutes(1)).unix_timestamp(),
            fee_receiver_relayer: Pubkey::new_unique(),
            relayer_signer: service.config.chain_config.express_relay.relayer.pubkey(),
        })
        .unwrap();
        let program = match opportunity.program.clone() {
            OpportunitySvmProgram::Swap(program) => program,
            _ => panic!("Expected swap program"),
        };
        let ata = &get_associated_token_address(
            &program.user_wallet_address,
            &spl_token::native_mint::id(),
        );
        let transfer_instruction = system_instruction::transfer(
            &program.user_wallet_address,
            &get_associated_token_address(
                &program.user_wallet_address,
                &spl_token::native_mint::id(),
            ),
            opportunity.buy_tokens[0].amount,
        );
        let sync_native_instruction =
            spl_token::instruction::sync_native(&spl_token::id(), ata).unwrap();


        let result = get_verify_bid_result(
            service,
            searcher,
            vec![
                transfer_instruction,
                sync_native_instruction,
                swap_instruction, // <--- no user close account instruction
            ],
            opportunity,
        )
        .await;

        assert_eq!(
            result.unwrap_err(),
            RestError::InvalidInstruction(
                None,
                InstructionError::InvalidCloseAccountInstructionsCount,
            )
        );
    }

    #[tokio::test]
    async fn test_verify_bid_when_multiple_close_account_instructions() {
        let (service, opportunities) = get_service(false);
        let searcher = Keypair::new();
        let opportunity = opportunities[3].clone(); // Searcher token wsol
        let swap_instruction = svm::Svm::get_swap_instruction(GetSwapInstructionParams {
            searcher:             searcher.pubkey(),
            opportunity_params:   get_opportunity_params(opportunity.clone()),
            bid_amount:           1,
            deadline:             (OffsetDateTime::now_utc() + Duration::minutes(1))
                .unix_timestamp(),
            fee_receiver_relayer: Pubkey::new_unique(),
            relayer_signer:       service.config.chain_config.express_relay.relayer.pubkey(),
        })
        .unwrap();
        let program = match opportunity.program.clone() {
            OpportunitySvmProgram::Swap(program) => program,
            _ => panic!("Expected swap program"),
        };
        let ata = &get_associated_token_address(
            &program.user_wallet_address,
            &spl_token::native_mint::id(),
        );
        let close_account_instruction = spl_token::instruction::close_account(
            &spl_token::id(),
            ata,
            &program.user_wallet_address,
            &program.user_wallet_address,
            &[],
        )
        .unwrap();
        let result = get_verify_bid_result(
            service,
            searcher,
            vec![
                swap_instruction,
                close_account_instruction.clone(),
                close_account_instruction,
            ],
            opportunity,
        )
        .await;
        assert_eq!(
            result.unwrap_err(),
            RestError::InvalidInstruction(
                None,
                InstructionError::InvalidCloseAccountInstructionsCount,
            )
        );
    }

    #[tokio::test]
    async fn test_verify_bid_when_no_close_account_instructions() {
        let (service, opportunities) = get_service(false);
        let searcher = Keypair::new();
        let opportunity = opportunities[3].clone(); // Searcher token wsol
        let swap_instruction = svm::Svm::get_swap_instruction(GetSwapInstructionParams {
            searcher:             searcher.pubkey(),
            opportunity_params:   get_opportunity_params(opportunity.clone()),
            bid_amount:           1,
            deadline:             (OffsetDateTime::now_utc() + Duration::minutes(1))
                .unix_timestamp(),
            fee_receiver_relayer: Pubkey::new_unique(),
            relayer_signer:       service.config.chain_config.express_relay.relayer.pubkey(),
        })
        .unwrap();
        let result =
            get_verify_bid_result(service, searcher, vec![swap_instruction], opportunity).await;
        assert_eq!(
            result.unwrap_err(),
            RestError::InvalidInstruction(
                None,
                InstructionError::InvalidCloseAccountInstructionsCount,
            )
        );
    }

    #[tokio::test]
    async fn test_verify_bid_when_invalid_account_to_close() {
        let (service, opportunities) = get_service(false);
        let searcher = Keypair::new();
        let opportunity = opportunities[3].clone(); // Searcher token wsol
        let swap_instruction = svm::Svm::get_swap_instruction(GetSwapInstructionParams {
            searcher:             searcher.pubkey(),
            opportunity_params:   get_opportunity_params(opportunity.clone()),
            bid_amount:           1,
            deadline:             (OffsetDateTime::now_utc() + Duration::minutes(1))
                .unix_timestamp(),
            fee_receiver_relayer: Pubkey::new_unique(),
            relayer_signer:       service.config.chain_config.express_relay.relayer.pubkey(),
        })
        .unwrap();
        let program = match opportunity.program.clone() {
            OpportunitySvmProgram::Swap(program) => program,
            _ => panic!("Expected swap program"),
        };
        let found = searcher.pubkey();
        let close_account_instruction = spl_token::instruction::close_account(
            &spl_token::id(),
            &found,
            &program.user_wallet_address,
            &program.user_wallet_address,
            &[],
        )
        .unwrap();
        let result = get_verify_bid_result(
            service,
            searcher,
            vec![swap_instruction, close_account_instruction],
            opportunity,
        )
        .await;
        assert_eq!(
            result.unwrap_err(),
            RestError::InvalidInstruction(
                None,
                InstructionError::InvalidCloseAccountInstructionsCount
            )
        );
    }

    #[tokio::test]
    async fn test_verify_bid_when_invalid_close_account_destination() {
        let (service, opportunities) = get_service(false);
        let searcher = Keypair::new();
        let opportunity = opportunities[3].clone(); // Searcher token wsol
        let swap_instruction = svm::Svm::get_swap_instruction(GetSwapInstructionParams {
            searcher:             searcher.pubkey(),
            opportunity_params:   get_opportunity_params(opportunity.clone()),
            bid_amount:           1,
            deadline:             (OffsetDateTime::now_utc() + Duration::minutes(1))
                .unix_timestamp(),
            fee_receiver_relayer: Pubkey::new_unique(),
            relayer_signer:       service.config.chain_config.express_relay.relayer.pubkey(),
        })
        .unwrap();
        let program = match opportunity.program.clone() {
            OpportunitySvmProgram::Swap(program) => program,
            _ => panic!("Expected swap program"),
        };
        let ata = &get_associated_token_address(
            &program.user_wallet_address,
            &spl_token::native_mint::id(),
        );
        let found = Pubkey::new_unique();
        let close_account_instruction = spl_token::instruction::close_account(
            &spl_token::id(),
            ata,
            &found,
            &program.user_wallet_address,
            &[],
        )
        .unwrap();
        let result = get_verify_bid_result(
            service,
            searcher,
            vec![swap_instruction, close_account_instruction],
            opportunity,
        )
        .await;
        assert_eq!(
            result.unwrap_err(),
            RestError::InvalidInstruction(
                None,
                InstructionError::InvalidDestinationCloseAccountInstruction {
                    expected: program.user_wallet_address,
                    found
                }
            )
        );
    }

    #[tokio::test]
    async fn test_verify_bid_searcher_wsol() {
        let (service, opportunities) = get_service(true);
        let opportunity = opportunities[3].clone(); // Searcher token wsol
        let bid_amount = 1;
        let searcher = Keypair::new();
        let swap_instruction = svm::Svm::get_swap_instruction(GetSwapInstructionParams {
            searcher: searcher.pubkey(),
            opportunity_params: get_opportunity_params(opportunity.clone()),
            bid_amount,
            deadline: (OffsetDateTime::now_utc() + Duration::minutes(1)).unix_timestamp(),
            fee_receiver_relayer: Pubkey::new_unique(),
            relayer_signer: service.config.chain_config.express_relay.relayer.pubkey(),
        })
        .unwrap();
        let program = match opportunity.program.clone() {
            OpportunitySvmProgram::Swap(program) => program,
            _ => panic!("Expected swap program"),
        };
        let ata = &get_associated_token_address(
            &program.user_wallet_address,
            &spl_token::native_mint::id(),
        );
        let close_account_instruction = spl_token::instruction::close_account(
            &spl_token::id(),
            ata,
            &program.user_wallet_address,
            &program.user_wallet_address,
            &[],
        )
        .unwrap();
        let mut transaction = Transaction::new_with_payer(
            &[swap_instruction, close_account_instruction],
            Some(&searcher.pubkey()),
        );
        transaction.partial_sign(&[searcher], Hash::default());
        let bid_create = BidCreate::<Svm> {
            chain_id:        service.config.chain_id.clone(),
            initiation_time: OffsetDateTime::now_utc(),
            profile:         None,
            chain_data:      BidChainDataCreateSvm::Swap(BidChainDataSwapCreateSvm {
                opportunity_id: opportunity.core_fields.id,
                transaction:    transaction.clone().into(),
            }),
        };

        let result = service
            .verify_bid(super::VerifyBidInput { bid_create })
            .await
            .unwrap();
        let swap_params = get_opportunity_swap_params(opportunity);
        assert_eq!(
            result.0,
            BidChainDataSvm {
                transaction:                  transaction.into(),
                permission_account:           swap_params.permission_account,
                router:                       swap_params.router_account,
                bid_payment_instruction_type: BidPaymentInstructionType::Swap,
            }
        );
        assert_eq!(result.1, bid_amount);
    }

    #[tokio::test]
    async fn test_verify_bid_when_invalid_associated_router_token_account() {
        let (service, opportunities) = get_service(true);
        let mut opportunity = opportunities[0].clone();
        let expected = get_associated_token_address(
            &opportunity.router,
            &opportunity.core_fields.buy_tokens[0].token,
        );
        opportunity.router = Pubkey::new_unique();
        let found = get_associated_token_address(
            &opportunity.router,
            &opportunity.core_fields.buy_tokens[0].token,
        );
        let bid_amount = 1;
        let searcher = Keypair::new();
        let swap_instruction = svm::Svm::get_swap_instruction(GetSwapInstructionParams {
            searcher: searcher.pubkey(),
            opportunity_params: get_opportunity_params(opportunity.clone()),
            bid_amount,
            deadline: (OffsetDateTime::now_utc() + Duration::minutes(1)).unix_timestamp(),
            fee_receiver_relayer: Pubkey::new_unique(),
            relayer_signer: service.config.chain_config.express_relay.relayer.pubkey(),
        })
        .unwrap();
        let result = get_verify_bid_result(
            service,
            searcher,
            vec![swap_instruction],
            opportunities[0].clone(),
        )
        .await;
        assert_eq!(
            result.unwrap_err(),
            RestError::InvalidSwapInstruction(SwapInstructionError::AssociatedRouterTokenAccount {
                expected,
                found,
            },),
        );
    }

    #[tokio::test]
    async fn test_verify_bid_when_invalid_deadline() {
        let (service, opportunities) = get_service(true);

        let bid_amount = 1;
        let searcher = Keypair::new();
        let deadline = (OffsetDateTime::now_utc() + BID_MINIMUM_LIFE_TIME_SVM_OTHER
            - Duration::seconds(1))
        .unix_timestamp();
        let instruction = svm::Svm::get_swap_instruction(GetSwapInstructionParams {
            searcher: searcher.pubkey(),
            opportunity_params: get_opportunity_params(opportunities[0].clone()),
            bid_amount,
            deadline,
            fee_receiver_relayer: Pubkey::new_unique(),
            relayer_signer: service.config.chain_config.express_relay.relayer.pubkey(),
        })
        .unwrap();
        let mut transaction = Transaction::new_with_payer(&[instruction], Some(&searcher.pubkey()));
        transaction.partial_sign(&[searcher], Hash::default());

        let bid_create = BidCreate::<Svm> {
            chain_id:        service.config.chain_id.clone(),
            initiation_time: OffsetDateTime::now_utc(),
            profile:         None,
            chain_data:      BidChainDataCreateSvm::Swap(BidChainDataSwapCreateSvm {
                opportunity_id: opportunities[0].core_fields.id,
                transaction:    transaction.clone().into(),
            }),
        };

        let result = service
            .verify_bid(super::VerifyBidInput { bid_create })
            .await;
        assert_eq!(
            result.unwrap_err(),
            RestError::InvalidDeadline {
                deadline: OffsetDateTime::from_unix_timestamp(deadline).unwrap(),
                minimum:  BID_MINIMUM_LIFE_TIME_SVM_OTHER,
            },
        )
    }

    #[tokio::test]
    async fn test_verify_bid_when_invalid_searcher_signature() {
        let (service, opportunities) = get_service(true);

        let bid_amount = 1;
        let searcher = Keypair::new();
        let instruction = svm::Svm::get_swap_instruction(GetSwapInstructionParams {
            searcher: searcher.pubkey(),
            opportunity_params: get_opportunity_params(opportunities[0].clone()),
            bid_amount,
            deadline: (OffsetDateTime::now_utc() + Duration::minutes(1)).unix_timestamp(),
            fee_receiver_relayer: Pubkey::new_unique(),
            relayer_signer: service.config.chain_config.express_relay.relayer.pubkey(),
        })
        .unwrap();
        let transaction = Transaction::new_with_payer(&[instruction], Some(&searcher.pubkey()));

        let bid_create = BidCreate::<Svm> {
            chain_id:        service.config.chain_id.clone(),
            initiation_time: OffsetDateTime::now_utc(),
            profile:         None,
            chain_data:      BidChainDataCreateSvm::Swap(BidChainDataSwapCreateSvm {
                opportunity_id: opportunities[0].core_fields.id,
                transaction:    transaction.clone().into(),
            }),
        };

        let result = service
            .verify_bid(super::VerifyBidInput { bid_create })
            .await;
        assert_eq!(
            result.unwrap_err(),
            RestError::InvalidSignature(searcher.pubkey())
        )
    }

    #[tokio::test]
    async fn test_verify_bid_when_no_relayer_signer() {
        let (service, opportunities) = get_service(true);

        let bid_amount = 1;
        let searcher = Keypair::new();
        let mut instruction = svm::Svm::get_swap_instruction(GetSwapInstructionParams {
            searcher: searcher.pubkey(),
            opportunity_params: get_opportunity_params(opportunities[0].clone()),
            bid_amount,
            deadline: (OffsetDateTime::now_utc() + Duration::minutes(1)).unix_timestamp(),
            fee_receiver_relayer: Pubkey::new_unique(),
            relayer_signer: service.config.chain_config.express_relay.relayer.pubkey(),
        })
        .unwrap();
        for index in 0..instruction.accounts.len() {
            if instruction.accounts[index].pubkey
                == service.config.chain_config.express_relay.relayer.pubkey()
            {
                instruction.accounts[index] = AccountMeta {
                    is_signer: false,
                    ..instruction.accounts[index]
                };
            }
        }
        let mut transaction = Transaction::new_with_payer(&[instruction], Some(&searcher.pubkey()));
        transaction.partial_sign(&[searcher], Hash::default());

        let bid_create = BidCreate::<Svm> {
            chain_id:        service.config.chain_id.clone(),
            initiation_time: OffsetDateTime::now_utc(),
            profile:         None,
            chain_data:      BidChainDataCreateSvm::Swap(BidChainDataSwapCreateSvm {
                opportunity_id: opportunities[0].core_fields.id,
                transaction:    transaction.clone().into(),
            }),
        };

        let result = service
            .verify_bid(super::VerifyBidInput { bid_create })
            .await;
        assert_eq!(
            result.unwrap_err(),
            RestError::RelayerNotSigner(service.config.chain_config.express_relay.relayer.pubkey())
        )
    }

    #[tokio::test]
    async fn test_verify_bid_when_fee_payer_is_user() {
        let (service, opportunities) = get_service(true);

        let bid_amount = 1;
        let searcher = Keypair::new();
        let instruction = svm::Svm::get_swap_instruction(GetSwapInstructionParams {
            searcher: searcher.pubkey(),
            opportunity_params: get_opportunity_params(opportunities[0].clone()),
            bid_amount,
            deadline: (OffsetDateTime::now_utc() + Duration::minutes(1)).unix_timestamp(),
            fee_receiver_relayer: Pubkey::new_unique(),
            relayer_signer: service.config.chain_config.express_relay.relayer.pubkey(),
        })
        .unwrap();
        let program = match opportunities[0].program.clone() {
            OpportunitySvmProgram::Swap(program) => program,
            _ => panic!("Expected swap program"),
        };
        let mut transaction =
            Transaction::new_with_payer(&[instruction], Some(&program.user_wallet_address));
        transaction.partial_sign(&[searcher], Hash::default());

        let bid_create = BidCreate::<Svm> {
            chain_id:        service.config.chain_id.clone(),
            initiation_time: OffsetDateTime::now_utc(),
            profile:         None,
            chain_data:      BidChainDataCreateSvm::Swap(BidChainDataSwapCreateSvm {
                opportunity_id: opportunities[0].core_fields.id,
                transaction:    transaction.clone().into(),
            }),
        };

        let result = service
            .verify_bid(super::VerifyBidInput { bid_create })
            .await;
        assert_eq!(
            result.unwrap_err(),
            RestError::InvalidFirstSigner("Fee payer should not be user".to_string())
        )
    }

    #[tokio::test]
    async fn test_verify_bid_when_simulation_fails() {
        let (mut service, opportunities) = get_service(true);
        let mut rpc_client = MockRpcClient::new();
        rpc_client.expect_send().returning(|_, _| {
            Ok(serde_json::json!({
                "context": { "slot": 1 },
                "value": {
                    "err": "AccountInUse",
                    "accounts": null,
                    "logs": [],
                    "returnData": {
                        "data": ["", "base64"],
                        "programId": "11111111111111111111111111111111",
                    },
                    "unitsConsumed": 0
                }
            }))
        });
        let service_inner = Arc::get_mut(&mut service.0).unwrap();
        service_inner.config.chain_config.client =
            RpcClient::new_sender(rpc_client, RpcClientConfig::default());

        let bid_amount = 1;
        let searcher = Keypair::new();
        let instruction = svm::Svm::get_swap_instruction(GetSwapInstructionParams {
            searcher: searcher.pubkey(),
            opportunity_params: get_opportunity_params(opportunities[0].clone()),
            bid_amount,
            deadline: (OffsetDateTime::now_utc() + Duration::minutes(1)).unix_timestamp(),
            fee_receiver_relayer: Pubkey::new_unique(),
            relayer_signer: service.config.chain_config.express_relay.relayer.pubkey(),
        })
        .unwrap();
        let mut transaction = Transaction::new_with_payer(&[instruction], Some(&searcher.pubkey()));
        transaction.partial_sign(&[searcher], Hash::default());

        let bid_create = BidCreate::<Svm> {
            chain_id:        service.config.chain_id.clone(),
            initiation_time: OffsetDateTime::now_utc(),
            profile:         None,
            chain_data:      BidChainDataCreateSvm::Swap(BidChainDataSwapCreateSvm {
                opportunity_id: opportunities[0].core_fields.id,
                transaction:    transaction.clone().into(),
            }),
        };

        let result = service
            .verify_bid(super::VerifyBidInput { bid_create })
            .await;
        assert_eq!(
            result.unwrap_err(),
            RestError::SimulationError {
                result: Bytes::default(),
                reason: "".to_string(),
            }
        )
    }

    #[tokio::test]
    async fn test_verify_bid_when_duplicate() {
        let (mut service, opportunities) = get_service(true);
        let mut db = MockDatabase::<Svm>::default();
        db.expect_add_bid().returning(|_| Ok(()));
        let service_inner = Arc::get_mut(&mut service.0).unwrap();
        service_inner.repo = Arc::new(Repository::new(db, service_inner.config.chain_id.clone()));

        let bid_amount = 1;
        let searcher = Keypair::new();
        let instruction = svm::Svm::get_swap_instruction(GetSwapInstructionParams {
            searcher: searcher.pubkey(),
            opportunity_params: get_opportunity_params(opportunities[0].clone()),
            bid_amount,
            deadline: (OffsetDateTime::now_utc() + Duration::minutes(1)).unix_timestamp(),
            fee_receiver_relayer: Pubkey::new_unique(),
            relayer_signer: service.config.chain_config.express_relay.relayer.pubkey(),
        })
        .unwrap();
        let mut transaction = Transaction::new_with_payer(&[instruction], Some(&searcher.pubkey()));
        transaction.partial_sign(&[searcher], Hash::default());

        let bid_create = BidCreate::<Svm> {
            chain_id:        service.config.chain_id.clone(),
            initiation_time: OffsetDateTime::now_utc(),
            profile:         None,
            chain_data:      BidChainDataCreateSvm::Swap(BidChainDataSwapCreateSvm {
                opportunity_id: opportunities[0].core_fields.id,
                transaction:    transaction.clone().into(),
            }),
        };
        let result = service
            .verify_bid(super::VerifyBidInput {
                bid_create: bid_create.clone(),
            })
            .await
            .unwrap();
        service
            .repo
            .add_bid(bid_create.clone(), &result.0, &result.1)
            .await
            .unwrap();
        let result = service
            .verify_bid(super::VerifyBidInput { bid_create })
            .await;
        assert_eq!(result.unwrap_err(), RestError::DuplicateBid,);
    }

    #[tokio::test]
    async fn test_verify_bid_when_invalid_close_account_owner() {
        let (service, opportunities) = get_service(false);
        let searcher = Keypair::new();
        let opportunity = opportunities[3].clone(); // Searcher token wsol
        let swap_instruction = svm::Svm::get_swap_instruction(GetSwapInstructionParams {
            searcher:             searcher.pubkey(),
            opportunity_params:   get_opportunity_params(opportunity.clone()),
            bid_amount:           1,
            deadline:             (OffsetDateTime::now_utc() + Duration::minutes(1))
                .unix_timestamp(),
            fee_receiver_relayer: Pubkey::new_unique(),
            relayer_signer:       service.config.chain_config.express_relay.relayer.pubkey(),
        })
        .unwrap();
        let program = match opportunity.program.clone() {
            OpportunitySvmProgram::Swap(program) => program,
            _ => panic!("Expected swap program"),
        };
        let ata = &get_associated_token_address(
            &program.user_wallet_address,
            &spl_token::native_mint::id(),
        );
        let found = Pubkey::new_unique();
        let close_account_instruction = spl_token::instruction::close_account(
            &spl_token::id(),
            ata,
            &program.user_wallet_address,
            &found,
            &[],
        )
        .unwrap();
        let result = get_verify_bid_result(
            service,
            searcher,
            vec![swap_instruction, close_account_instruction],
            opportunity,
        )
        .await;
        assert_eq!(
            result.unwrap_err(),
            RestError::InvalidInstruction(
                None,
                InstructionError::InvalidOwnerCloseAccountInstruction {
                    expected: program.user_wallet_address,
                    found
                }
            )
        );
    }

    #[tokio::test]
    async fn test_verify_bid_user_wsol_searcher_unwrap() {
        let (service, opportunities) = get_service(true);
        let opportunity = opportunities[2].clone(); // User token wsol
        let bid_amount = 1;
        let searcher = Keypair::new();
        let swap_instruction = svm::Svm::get_swap_instruction(GetSwapInstructionParams {
            searcher: searcher.pubkey(),
            opportunity_params: get_opportunity_params(opportunity.clone()),
            bid_amount,
            deadline: (OffsetDateTime::now_utc() + Duration::minutes(1)).unix_timestamp(),
            fee_receiver_relayer: Pubkey::new_unique(),
            relayer_signer: service.config.chain_config.express_relay.relayer.pubkey(),
        })
        .unwrap();
        let program = match opportunity.program.clone() {
            OpportunitySvmProgram::Swap(program) => program,
            _ => panic!("Expected swap program"),
        };
        let ata = &get_associated_token_address(
            &program.user_wallet_address,
            &spl_token::native_mint::id(),
        );
        let transfer_instruction = system_instruction::transfer(
            &program.user_wallet_address,
            &get_associated_token_address(
                &program.user_wallet_address,
                &spl_token::native_mint::id(),
            ),
            opportunity.buy_tokens[0].amount,
        );
        let sync_native_instruction =
            spl_token::instruction::sync_native(&spl_token::id(), ata).unwrap();
        let searcher_close_account_instruction = spl_token::instruction::close_account(
            &spl_token::id(),
            &get_associated_token_address(&searcher.pubkey(), &spl_token::native_mint::id()),
            &searcher.pubkey(),
            &searcher.pubkey(),
            &[],
        )
        .unwrap();
        let user_close_account_instruction = spl_token::instruction::close_account(
            &spl_token::id(),
            &get_associated_token_address(
                &program.user_wallet_address,
                &spl_token::native_mint::id(),
            ),
            &program.user_wallet_address,
            &program.user_wallet_address,
            &[],
        )
        .unwrap();
        let mut transaction = Transaction::new_with_payer(
            &[
                transfer_instruction,
                sync_native_instruction,
                swap_instruction,
                searcher_close_account_instruction,
                user_close_account_instruction,
            ],
            Some(&searcher.pubkey()),
        );
        transaction.partial_sign(&[searcher], Hash::default());
        let bid_create = BidCreate::<Svm> {
            chain_id:        service.config.chain_id.clone(),
            initiation_time: OffsetDateTime::now_utc(),
            profile:         None,
            chain_data:      BidChainDataCreateSvm::Swap(BidChainDataSwapCreateSvm {
                opportunity_id: opportunity.core_fields.id,
                transaction:    transaction.clone().into(),
            }),
        };

        let result = service
            .verify_bid(super::VerifyBidInput { bid_create })
            .await
            .unwrap();
        let swap_params = get_opportunity_swap_params(opportunity);
        assert_eq!(
            result.0,
            BidChainDataSvm {
                transaction:                  transaction.into(),
                permission_account:           swap_params.permission_account,
                router:                       swap_params.router_account,
                bid_payment_instruction_type: BidPaymentInstructionType::Swap,
            }
        );
        assert_eq!(result.1, bid_amount);
    }

    #[tokio::test]
    async fn test_verify_bid_when_invalid_searcher_account_to_close() {
        let (service, opportunities) = get_service(true);
        let opportunity = opportunities[2].clone(); // User token wsol
        let bid_amount = 1;
        let searcher = Keypair::new();
        let swap_instruction = svm::Svm::get_swap_instruction(GetSwapInstructionParams {
            searcher: searcher.pubkey(),
            opportunity_params: get_opportunity_params(opportunity.clone()),
            bid_amount,
            deadline: (OffsetDateTime::now_utc() + Duration::minutes(1)).unix_timestamp(),
            fee_receiver_relayer: Pubkey::new_unique(),
            relayer_signer: service.config.chain_config.express_relay.relayer.pubkey(),
        })
        .unwrap();
        let program = match opportunity.program.clone() {
            OpportunitySvmProgram::Swap(program) => program,
            _ => panic!("Expected swap program"),
        };
        let ata = &get_associated_token_address(
            &program.user_wallet_address,
            &spl_token::native_mint::id(),
        );
        let transfer_instruction = system_instruction::transfer(
            &program.user_wallet_address,
            &get_associated_token_address(
                &program.user_wallet_address,
                &spl_token::native_mint::id(),
            ),
            opportunity.buy_tokens[0].amount,
        );
        let sync_native_instruction =
            spl_token::instruction::sync_native(&spl_token::id(), ata).unwrap();
        let user_close_account_instruction = spl_token::instruction::close_account(
            &spl_token::id(),
            &get_associated_token_address(
                &program.user_wallet_address,
                &spl_token::native_mint::id(),
            ),
            &program.user_wallet_address,
            &program.user_wallet_address,
            &[],
        )
        .unwrap();
        let found =
            get_associated_token_address(&Pubkey::new_unique(), &spl_token::native_mint::id());
        let expected =
            get_associated_token_address(&searcher.pubkey(), &spl_token::native_mint::id());
        let searcher_close_account_instruction = spl_token::instruction::close_account(
            &spl_token::id(),
            &found,
            &searcher.pubkey(),
            &searcher.pubkey(),
            &[],
        )
        .unwrap();
        let result = get_verify_bid_result(
            service,
            searcher,
            vec![
                transfer_instruction,
                sync_native_instruction,
                swap_instruction,
                searcher_close_account_instruction,
                user_close_account_instruction,
            ],
            opportunity,
        )
        .await;
        assert_eq!(
            result.unwrap_err(),
            RestError::InvalidInstruction(
                None,
                InstructionError::InvalidAccountToCloseCloseAccountInstruction { expected, found }
            )
        );
    }

    #[tokio::test]
    async fn test_verify_bid_searcher_wsol_searcher_wrap() {
        let (service, opportunities) = get_service(true);
        let opportunity = opportunities[3].clone(); // Searcher token wsol
        let bid_amount = 1;
        let searcher = Keypair::new();
        let swap_instruction = svm::Svm::get_swap_instruction(GetSwapInstructionParams {
            searcher: searcher.pubkey(),
            opportunity_params: get_opportunity_params(opportunity.clone()),
            bid_amount,
            deadline: (OffsetDateTime::now_utc() + Duration::minutes(1)).unix_timestamp(),
            fee_receiver_relayer: Pubkey::new_unique(),
            relayer_signer: service.config.chain_config.express_relay.relayer.pubkey(),
        })
        .unwrap();
        let program = match opportunity.program.clone() {
            OpportunitySvmProgram::Swap(program) => program,
            _ => panic!("Expected swap program"),
        };
        let ata = get_associated_token_address(
            &program.user_wallet_address,
            &spl_token::native_mint::id(),
        );
        let close_account_instruction = spl_token::instruction::close_account(
            &spl_token::id(),
            &ata,
            &program.user_wallet_address,
            &program.user_wallet_address,
            &[],
        )
        .unwrap();
        let searcher_ata =
            get_associated_token_address(&searcher.pubkey(), &spl_token::native_mint::id());
        let transfer_instruction_searcher = system_instruction::transfer(
            &searcher.pubkey(),
            &searcher_ata,
            opportunity.buy_tokens[0].amount,
        );
        let sync_native_instruction_searcher =
            spl_token::instruction::sync_native(&spl_token::id(), &searcher_ata).unwrap();
        let mut transaction = Transaction::new_with_payer(
            &[
                transfer_instruction_searcher,
                sync_native_instruction_searcher,
                swap_instruction,
                close_account_instruction,
            ],
            Some(&searcher.pubkey()),
        );
        transaction.partial_sign(&[searcher], Hash::default());
        let bid_create = BidCreate::<Svm> {
            chain_id:        service.config.chain_id.clone(),
            initiation_time: OffsetDateTime::now_utc(),
            profile:         None,
            chain_data:      BidChainDataCreateSvm::Swap(BidChainDataSwapCreateSvm {
                opportunity_id: opportunity.core_fields.id,
                transaction:    transaction.clone().into(),
            }),
        };

        let result = service
            .verify_bid(super::VerifyBidInput { bid_create })
            .await
            .unwrap();
        let swap_params = get_opportunity_swap_params(opportunity);
        assert_eq!(
            result.0,
            BidChainDataSvm {
                transaction:                  transaction.into(),
                permission_account:           swap_params.permission_account,
                router:                       swap_params.router_account,
                bid_payment_instruction_type: BidPaymentInstructionType::Swap,
            }
        );
        assert_eq!(result.1, bid_amount);
    }

    #[tokio::test]
    async fn test_verify_bid_when_invalid_searcher_account_from_transfer() {
        let (service, opportunities) = get_service(true);
        let opportunity = opportunities[3].clone(); // Searcher token wsol
        let bid_amount = 1;
        let searcher = Keypair::new();
        let swap_instruction = svm::Svm::get_swap_instruction(GetSwapInstructionParams {
            searcher: searcher.pubkey(),
            opportunity_params: get_opportunity_params(opportunity.clone()),
            bid_amount,
            deadline: (OffsetDateTime::now_utc() + Duration::minutes(1)).unix_timestamp(),
            fee_receiver_relayer: Pubkey::new_unique(),
            relayer_signer: service.config.chain_config.express_relay.relayer.pubkey(),
        })
        .unwrap();
        let program = match opportunity.program.clone() {
            OpportunitySvmProgram::Swap(program) => program,
            _ => panic!("Expected swap program"),
        };
        let ata = get_associated_token_address(
            &program.user_wallet_address,
            &spl_token::native_mint::id(),
        );
        let close_account_instruction = spl_token::instruction::close_account(
            &spl_token::id(),
            &ata,
            &program.user_wallet_address,
            &program.user_wallet_address,
            &[],
        )
        .unwrap();
        let searcher_ata =
            get_associated_token_address(&searcher.pubkey(), &spl_token::native_mint::id());
        let expected = searcher.pubkey();
        let found = Pubkey::new_unique();
        let transfer_instruction_searcher =
            system_instruction::transfer(&found, &searcher_ata, opportunity.buy_tokens[0].amount);
        let sync_native_instruction_searcher =
            spl_token::instruction::sync_native(&spl_token::id(), &searcher_ata).unwrap();
        let result = get_verify_bid_result(
            service,
            searcher,
            vec![
                transfer_instruction_searcher,
                sync_native_instruction_searcher,
                swap_instruction,
                close_account_instruction,
            ],
            opportunity,
        )
        .await;
        assert_eq!(
            result.unwrap_err(),
            RestError::InvalidInstruction(
                None,
                InstructionError::InvalidFromAccountTransferInstruction { expected, found }
            )
        );
    }

    #[tokio::test]
    async fn test_verify_bid_when_invalid_searcher_account_to_transfer() {
        let (service, opportunities) = get_service(true);
        let opportunity = opportunities[3].clone(); // Searcher token wsol
        let bid_amount = 1;
        let searcher = Keypair::new();
        let swap_instruction = svm::Svm::get_swap_instruction(GetSwapInstructionParams {
            searcher: searcher.pubkey(),
            opportunity_params: get_opportunity_params(opportunity.clone()),
            bid_amount,
            deadline: (OffsetDateTime::now_utc() + Duration::minutes(1)).unix_timestamp(),
            fee_receiver_relayer: Pubkey::new_unique(),
            relayer_signer: service.config.chain_config.express_relay.relayer.pubkey(),
        })
        .unwrap();
        let program = match opportunity.program.clone() {
            OpportunitySvmProgram::Swap(program) => program,
            _ => panic!("Expected swap program"),
        };
        let ata = get_associated_token_address(
            &program.user_wallet_address,
            &spl_token::native_mint::id(),
        );
        let close_account_instruction = spl_token::instruction::close_account(
            &spl_token::id(),
            &ata,
            &program.user_wallet_address,
            &program.user_wallet_address,
            &[],
        )
        .unwrap();
        let expected =
            get_associated_token_address(&searcher.pubkey(), &spl_token::native_mint::id());
        let found = Pubkey::new_unique();
        let transfer_instruction_searcher = system_instruction::transfer(
            &searcher.pubkey(),
            &found,
            opportunity.buy_tokens[0].amount,
        );
        let sync_native_instruction_searcher =
            spl_token::instruction::sync_native(&spl_token::id(), &found).unwrap();
        let result = get_verify_bid_result(
            service,
            searcher,
            vec![
                transfer_instruction_searcher,
                sync_native_instruction_searcher,
                swap_instruction,
                close_account_instruction,
            ],
            opportunity,
        )
        .await;
        assert_eq!(
            result.unwrap_err(),
            RestError::InvalidInstruction(
                None,
                InstructionError::InvalidToAccountTransferInstruction { expected, found }
            )
        );
    }
}
