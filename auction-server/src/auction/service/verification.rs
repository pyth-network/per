use {
    super::{
        auction_manager::TOTAL_BIDS_PER_AUCTION_EVM,
        ChainTrait,
        Service,
    },
    crate::{
        api::RestError,
        auction::{
            entities::{
                self,
                BidChainData,
                BidChainDataCreateSvm,
                BidPaymentInstructionType,
                SubmitType,
            },
            service::get_live_bids::GetLiveBidsInput,
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
        transaction::VersionedTransaction,
    },
    std::{
        str::FromStr,
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

struct BidDataSvm {
    amount:             u64,
    router:             Pubkey,
    permission_account: Pubkey,
    deadline:           OffsetDateTime,
    submit_type:        SubmitType,
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
        submit_bid_instruction: &CompiledInstruction,
        position: usize,
    ) -> Result<Pubkey, RestError> {
        let static_accounts = tx.message.static_account_keys();
        let tx_lookup_tables = tx.message.address_table_lookups();

        let account_position = submit_bid_instruction
            .accounts
            .get(position)
            .ok_or_else(|| {
                RestError::BadParameters("Account not found in submit_bid instruction".to_string())
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
                let program_id = instruction.program_id(transaction.message.static_account_keys());
                *program_id == self.config.chain_config.express_relay.program_id
            })
            .cloned()
            .collect::<Vec<CompiledInstruction>>();

        let instruction = match instructions.len() {
            1 => Ok(instructions[0].clone()),
            _ => Err(RestError::BadParameters(format!("Bid must include exactly one instruction to Express Relay program but found {} instructions", instructions.len()))),
        }?;
        if !instruction.data.starts_with(&discriminator) {
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
                    return Err(RestError::BadParameters(format!(
                        "Bid deadline of {:?} is too short, bid must be valid for at least {:?} seconds",
                        deadline, min_life_time
                    )));
                }

                Ok(())
            }
            None => Ok(()),
        }
    }

    async fn extract_bid_data(
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
                let swap_instruction = self.extract_express_relay_instruction(
                    bid_data.transaction.clone(),
                    BidPaymentInstructionType::Swap,
                )?;
                let swap_data = Self::extract_swap_data(&swap_instruction)?;

                let user_wallet = self
                    .extract_account(
                        &bid_data.transaction,
                        &swap_instruction,
                        svm_config
                            .swap_instruction_account_positions
                            .user_wallet_account,
                    )
                    .await?;

                let mint_input = self
                    .extract_account(
                        &bid_data.transaction,
                        &swap_instruction,
                        svm_config
                            .swap_instruction_account_positions
                            .mint_input_account,
                    )
                    .await?;
                let mint_output = self
                    .extract_account(
                        &bid_data.transaction,
                        &swap_instruction,
                        svm_config
                            .swap_instruction_account_positions
                            .mint_output_account,
                    )
                    .await?;

                let opp = self
                    .opportunity_service
                    .get_live_opportunity_by_id(GetLiveOpportunityByIdInput {
                        opportunity_id: bid_data.opportunity_id,
                    })
                    .await
                    .ok_or(RestError::BadParameters(
                        "No swap opportunity with the given id found".to_string(),
                    ))?;

                let opp_swap_data = match opp.program {
                    Swap(opp_swap_data) => opp_swap_data,
                    _ => {
                        return Err(RestError::BadParameters(format!(
                            "Opportunity with id {} is not a swap opportunity",
                            bid_data.opportunity_id
                        )));
                    }
                };
                let (
                    bid_amount,
                    expected_input_token,
                    expected_input_amount,
                    expected_output_token,
                    expected_output_amount,
                ) = match opp_swap_data.quote_tokens.clone() {
                    QuoteTokens::InputTokenSpecified {
                        input_token,
                        output_token,
                        ..
                    } => (
                        swap_data.amount_output,
                        input_token.token,
                        Some(input_token.amount),
                        output_token,
                        None,
                    ),
                    QuoteTokens::OutputTokenSpecified {
                        input_token,
                        output_token,
                        ..
                    } => (
                        swap_data.amount_input,
                        input_token,
                        None,
                        output_token.token,
                        Some(output_token.amount),
                    ),
                };
                if let Some(expected_input_amount) = expected_input_amount {
                    if expected_input_amount != swap_data.amount_input {
                        return Err(RestError::BadParameters(
                            format!(
                                "Input amount in swap opportunity {} does not match the input amount in the swap instruction {}",
                                expected_input_amount, swap_data.amount_input
                            ),
                        ));
                    }
                }
                if let Some(expected_output_amount) = expected_output_amount {
                    if expected_output_amount != swap_data.amount_output {
                        return Err(RestError::BadParameters(
                            format!(
                                "Output amount in swap opportunity {} does not match the output amount in the swap instruction {}",
                                expected_output_amount, swap_data.amount_output
                            ),
                        ));
                    }
                }
                if expected_input_token != mint_input {
                    return Err(RestError::BadParameters(
                        format!(
                            "Input token in swap opportunity {} does not match the input token in the swap instruction {}",
                            expected_input_token, mint_input
                        ),
                    ));
                }
                if expected_output_token != mint_output {
                    return Err(RestError::BadParameters(
                        format!(
                            "Output token in swap opportunity {} does not match the output token in the swap instruction {}",
                            expected_output_token, mint_output
                        ),
                    ));
                }


                if swap_data.referral_fee_bps != opp_swap_data.referral_fee_bps {
                    return Err(RestError::BadParameters(
                        format!(
                            "Referral fee bps in swap opportunity {} does not match the referral fee bps in the swap instruction {}",
                            opp_swap_data.referral_fee_bps, swap_data.referral_fee_bps
                        ),
                    ));
                }
                if user_wallet != opp_swap_data.user_wallet_address {
                    return Err(RestError::BadParameters(
                        format!(
                            "User wallet address in swap opportunity {} does not match the user wallet address in the swap instruction {}",
                            opp_swap_data.user_wallet_address, user_wallet
                        ),
                    ));
                }
                let (fee_token, fee_token_program) = match swap_data.fee_token {
                    FeeToken::Input => (mint_input, opp_swap_data.input_token_program),
                    FeeToken::Output => (mint_output, opp_swap_data.output_token_program),
                };
                if opp_swap_data.fee_token != swap_data.fee_token {
                    return Err(RestError::BadParameters(
                        format!(
                            "Fee token in swap opportunity {:?} does not match the fee token in the swap instruction {:?}",
                            opp_swap_data.fee_token, swap_data.fee_token
                        ),
                    ));
                }

                let associated_token_program = Pubkey::from_str(
                    "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL",
                )
                .map_err(|e| {
                    RestError::BadParameters(format!(
                        "Invalid associated token program address: {:?}",
                        e
                    ))
                })?;

                let expected_router_fee_receiver_ta = Pubkey::find_program_address(
                    &[
                        &opp.router.to_bytes(),
                        &fee_token_program.to_bytes(),
                        &fee_token.to_bytes(),
                    ],
                    &associated_token_program,
                )
                .0;
                let router_fee_receiver_ta = self
                    .extract_account(
                        &bid_data.transaction,
                        &swap_instruction,
                        self.config
                            .chain_config
                            .express_relay
                            .swap_instruction_account_positions
                            .router_token_account,
                    )
                    .await?;

                if router_fee_receiver_ta != expected_router_fee_receiver_ta {
                    return Err(RestError::BadParameters(
                        format!("Associated token account for router does not match. Expected: {:?} found: {:?}", expected_router_fee_receiver_ta, router_fee_receiver_ta),
                    ));
                }

                let permission_account = get_quote_virtual_permission_account(
                    &opp_swap_data.quote_tokens,
                    &user_wallet,
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
            return Err(RestError::BadParameters(format!(
                "Relayer account {} is not a signer in the transaction",
                relayer_pubkey
            )));
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
                return Err(RestError::BadParameters(format!(
                    "Signature for account {} is invalid",
                    pubkey
                )));
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
                self.all_signatures_exists(
                    &message_bytes,
                    accounts,
                    &signatures,
                    &opportunity.get_missing_signers(),
                )
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
        let compute_budget = self
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
                let program_id = instruction.program_id(transaction.message.static_account_keys());
                if program_id != &compute_budget::id() {
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
            return Err(RestError::BadParameters(
                "Multiple SetComputeUnitPrice instructions".to_string(),
            ));
        }
        if budgets.is_empty() && compute_budget > 0 {
            return Err(RestError::BadParameters(format!(
                "No SetComputeUnitPrice instruction. Minimum compute budget is {}",
                compute_budget
            )));
        }
        if let Some(budget) = budgets.first() {
            if *budget < compute_budget {
                return Err(RestError::BadParameters(format!(
                    "Compute budget is too low. Minimum compute budget is {}",
                    compute_budget
                )));
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
            // TODO*: we should verify all components of the swap here (token amounts, fee token side, referral fee)
            SubmitType::ByOther => BidPaymentInstructionType::Swap,
            SubmitType::Invalid => {
                return Err(RestError::BadParameters(
                    "Invalid submit type for bid".to_string(),
                ));
            }
        };
        let bid_chain_data = entities::BidChainDataSvm {
            permission_account: bid_data.permission_account,
            router: bid_data.router,
            bid_payment_instruction_type,
            transaction: transaction.clone(),
        };
        let permission_key = bid_chain_data.get_permission_key();
        tracing::Span::current().record("permission_key", bid_data.permission_account.to_string());
        self.check_deadline(&bid_data.submit_type, bid_data.deadline)
            .await?;
        self.verify_signatures(&bid, &bid_chain_data, &bid_data.submit_type)
            .await?;
        self.simulate_bid(&bid).await?;

        // Check if the bid is not duplicate
        let live_bids = self
            .get_live_bids(GetLiveBidsInput { permission_key })
            .await;
        if live_bids.iter().any(|b| bid == *b) {
            return Err(RestError::BadParameters("Duplicate bid".to_string()));
        }

        Ok((bid_chain_data, bid_data.amount))
    }
}
