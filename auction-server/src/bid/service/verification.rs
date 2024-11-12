use {
    super::Service,
    crate::{
        api::RestError,
        bid::{
            entities::{
                self,
                BidChainData,
            },
            service::get_live_bids::GetLiveBidsInput,
        },
        kernel::entities::{
            Evm,
            PermissionKey,
            PermissionKeySvm,
            Svm,
        },
        opportunity::{
            self as opportunity,
            service::get_live_opportunities::GetLiveOpportunitiesInput,
        },
    },
    ::express_relay::{
        self as express_relay_svm,
    },
    anchor_lang::{
        AnchorDeserialize,
        Discriminator,
    },
    axum::async_trait,
    solana_sdk::{
        address_lookup_table::state::AddressLookupTable,
        commitment_config::CommitmentConfig,
        instruction::CompiledInstruction,
        pubkey::Pubkey,
        signer::Signer,
        transaction::VersionedTransaction,
    },
    time::OffsetDateTime,
};

pub struct VerifyBidInput<T: entities::BidCreateTrait> {
    pub bid_create: entities::BidCreate<T>,
}

pub type VerificationResult<T> = (
    <T as entities::BidTrait>::ChainData,
    <T as entities::BidTrait>::BidAmount,
);

#[async_trait]
pub trait Verification<T: entities::BidCreateTrait + entities::BidTrait> {
    /// Verify the bid, and extract the chain data from the bid.
    async fn verify_bid(
        &self,
        input: VerifyBidInput<T>,
    ) -> Result<VerificationResult<T>, RestError>;
}

#[async_trait]
impl Verification<Evm> for Service<Evm> {
    // As we submit bids together for an auction, the bid is limited as follows:
    // 1. The bid amount should cover gas fees for all bids included in the submission.
    // 2. Depending on the maximum number of bids in the auction, the transaction size for the bid is limited.
    // 3. Depending on the maximum number of bids in the auction, the gas consumption for the bid is limited.
    async fn verify_bid(
        &self,
        _input: VerifyBidInput<Evm>,
    ) -> Result<VerificationResult<Evm>, RestError> {
        todo!()
    }
}

struct BidDataSvm {
    amount:             u64,
    router:             Pubkey,
    permission_account: Pubkey,
    deadline:           i64,
}

const BID_MINIMUM_LIFE_TIME_SVM_SERVER: i64 = 5;
const BID_MINIMUM_LIFE_TIME_SVM_OTHER: i64 = 10;

impl Service<Svm> {
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
            .ok_or_else(|| RestError::BadParameters("Account not found".to_string()))?;

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

    async fn extract_account_svm(
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

    fn extract_submit_bid_data(
        instruction: &CompiledInstruction,
    ) -> Result<express_relay_svm::SubmitBidArgs, RestError> {
        let discriminator = express_relay_svm::instruction::SubmitBid::discriminator();
        express_relay_svm::SubmitBidArgs::try_from_slice(
            &instruction.data.as_slice()[discriminator.len()..],
        )
        .map_err(|e| {
            RestError::BadParameters(format!("Invalid submit_bid instruction data: {}", e))
        })
    }

    // Checks that the transaction includes exactly one submit_bid instruction to the Express Relay on-chain program
    fn verify_submit_bid_instruction(
        &self,
        transaction: VersionedTransaction,
    ) -> Result<CompiledInstruction, RestError> {
        let submit_bid_instructions: Vec<CompiledInstruction> = transaction
            .message
            .instructions()
            .iter()
            .filter(|instruction| {
                let program_id = instruction.program_id(transaction.message.static_account_keys());
                if *program_id != self.config.chain_config.express_relay_program_id {
                    return false;
                }

                instruction
                    .data
                    .starts_with(&express_relay_svm::instruction::SubmitBid::discriminator())
            })
            .cloned()
            .collect();

        match submit_bid_instructions.len() {
            1 => Ok(submit_bid_instructions[0].clone()),
            _ => Err(RestError::BadParameters(
                "Bid has to include exactly one submit_bid instruction to Express Relay program"
                    .to_string(),
            )),
        }
    }

    async fn get_submission_state(&self, permission_key: PermissionKeySvm) -> entities::SubmitType {
        if permission_key.0.starts_with(
            &self
                .config
                .chain_config
                .wallet_program_router_account
                .to_bytes(),
        ) {
            // if self.opportunity_exists(store_new, permission_key).await {
            //     SubmitType::SubmitByOther
            // } else {
            //     SubmitType::Invalid
            // }
            entities::SubmitType::ByOther
        } else {
            entities::SubmitType::ByServer
        }
    }

    async fn check_deadline(
        &self,
        permission_key: PermissionKeySvm,
        deadline: i64,
    ) -> Result<(), RestError> {
        let minimum_bid_life_time = match self.get_submission_state(permission_key).await {
            entities::SubmitType::ByServer => Some(BID_MINIMUM_LIFE_TIME_SVM_SERVER),
            entities::SubmitType::ByOther => Some(BID_MINIMUM_LIFE_TIME_SVM_OTHER),
            entities::SubmitType::Invalid => None,
        };

        match minimum_bid_life_time {
            Some(min_life_time) => {
                let minimum_deadline = OffsetDateTime::now_utc().unix_timestamp() + min_life_time;
                // TODO: this uses the time at the server, which can lead to issues if Solana ever experiences clock drift
                // using the time at the server is not ideal, but the alternative is to make an RPC call to get the Solana block time
                // we should make this more robust, possibly by polling the current block time in the background
                if deadline < minimum_deadline {
                    return Err(RestError::BadParameters(format!(
                        "Bid deadline of {} is too short, bid must be valid for at least {} seconds",
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
        transaction: VersionedTransaction,
    ) -> Result<BidDataSvm, RestError> {
        let submit_bid_instruction = self.verify_submit_bid_instruction(transaction)?;
        let _submit_bid_data = Self::extract_submit_bid_data(&submit_bid_instruction)?;

        // let permission_account = extract_account_svm(
        //     &tx,
        //     &submit_bid_instruction,
        //     chain_store.express_relay_svm.permission_account_position,
        //     &chain_store.lookup_table_cache,
        //     client,
        // )
        // .await?;
        // let router_account = extract_account_svm(
        //     &tx,
        //     &submit_bid_instruction,
        //     chain_store.express_relay_svm.router_account_position,
        //     &chain_store.lookup_table_cache,
        //     client,
        // )
        // .await?;
        // let mut permission_key = [0; 64];
        // permission_key[..32].copy_from_slice(&router_account.to_bytes());
        // permission_key[32..].copy_from_slice(&permission_account.to_bytes());
        // Ok(BidDataSvm {
        //     amount:         submit_bid_data.bid_amount,
        //     permission_key: PermissionKeySvm(permission_key),
        //     deadline:       submit_bid_data.deadline,
        // })
        todo!()
    }

    async fn verify_signatures(
        &self,
        bid: &entities::BidCreate<Svm>,
        chain_data: &entities::BidChainDataSvm,
    ) -> Result<(), RestError> {
        let message_bytes = chain_data.transaction.message.serialize();
        let signatures = chain_data.transaction.signatures.clone();
        let accounts = chain_data.transaction.message.static_account_keys();
        let permission_key = chain_data.get_permission_key();
        let all_signature_exists = match self.get_submission_state(permission_key.clone()).await {
            entities::SubmitType::Invalid => {
                // TODO Look at the todo comment in get_quote.rs file in opportunity module
                return Err(RestError::BadParameters(format!(
                    "The permission key is not valid for auction anymore: {:?}",
                    permission_key
                )));
            }
            entities::SubmitType::ByOther => {
                let opportunities = self
                    .get_store()
                    .opportunity_service_svm
                    .get_live_opportunities(GetLiveOpportunitiesInput {
                        key: opportunity::entities::OpportunityKey(
                            bid.chain_id.clone(),
                            PermissionKey::from(permission_key.0),
                        ),
                    })
                    .await;
                opportunities.into_iter().any(|opportunity| {
                    let mut missing_signers = opportunity.get_missing_signers();
                    missing_signers.push(self.config.chain_config.relayer.pubkey());
                    Svm::all_signatures_exists(
                        &message_bytes,
                        accounts,
                        &signatures,
                        &missing_signers,
                    )
                })
            }
            entities::SubmitType::ByServer => Svm::all_signatures_exists(
                &message_bytes,
                accounts,
                &signatures,
                &[self.config.chain_config.relayer.pubkey()],
            ),
        };

        if !all_signature_exists {
            Err(RestError::BadParameters("Invalid signatures".to_string()))
        } else {
            Ok(())
        }
    }

    async fn simulate_bid(&self, bid: &entities::BidCreate<Svm>) -> Result<(), RestError> {
        let response = self
            .config
            .chain_config
            .client
            .simulate_transaction(&bid.chain_data.transaction)
            .await;
        let result = response.map_err(|e| {
            tracing::error!("Error while simulating bid: {:?}", e);
            RestError::TemporarilyUnavailable
        })?;
        match result.value.err {
            Some(err) => {
                tracing::error!(
                    error = ?err,
                    context = ?result.context,
                    "Error while simulating bid",
                );
                let mut msgs = result.value.logs.unwrap_or_default();
                msgs.push(err.to_string());
                Err(RestError::SimulationError {
                    result: Default::default(),
                    reason: msgs.join("\n"),
                })
            }
            None => Ok(()),
        }
    }
}

#[async_trait]
impl Verification<Svm> for Service<Svm> {
    async fn verify_bid(
        &self,
        input: VerifyBidInput<Svm>,
    ) -> Result<VerificationResult<Svm>, RestError> {
        let bid = input.bid_create;
        Svm::check_tx_size(&bid.chain_data.transaction)?;
        let bid_data = self
            .extract_bid_data(bid.chain_data.transaction.clone())
            .await?;
        let bid_chain_data = entities::BidChainDataSvm {
            permission_account: bid_data.permission_account,
            router:             bid_data.router,
            transaction:        bid.chain_data.transaction.clone(),
        };
        self.check_deadline(bid_chain_data.get_permission_key(), bid_data.deadline)
            .await?;
        self.verify_signatures(&bid, &bid_chain_data).await?;
        // TODO we should verify that the wallet bids also include another instruction to the swap program with the appropriate accounts and fields
        self.simulate_bid(&bid).await?;

        // Check if the bid is not duplicate
        let live_bids = self
            .get_live_bids(GetLiveBidsInput {
                permission_key: bid_chain_data.get_permission_key(),
            })
            .await;
        if live_bids.iter().any(|b| bid == *b) {
            return Err(RestError::BadParameters("Duplicate bid".to_string()));
        }

        Ok((bid_chain_data, bid_data.amount))
    }
}
