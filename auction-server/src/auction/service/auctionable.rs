use {
    super::{
        ChainTrait,
        Service,
    },
    crate::{
        api::RestError,
        auction::entities,
        kernel::entities::{
            Evm,
            Svm,
        },
        opportunity::{
            self,
            service::get_live_opportunities::GetLiveOpportunitiesInput,
        },
    },
    anyhow::Result,
    axum::async_trait,
    ethers::{
        providers::{
            Middleware,
            Provider,
            SubscriptionStream,
            Ws,
        },
        types::{
            Block,
            Bytes,
            TransactionReceipt,
            H256,
            U256,
        },
    },
    futures::Stream,
    solana_client::{
        nonblocking::pubsub_client::PubsubClient,
        rpc_config::RpcSendTransactionConfig,
        rpc_response::SlotInfo,
    },
    solana_sdk::{
        bs58,
        commitment_config::CommitmentConfig,
        signature::{
            Signature,
            Signer,
        },
        transaction::{
            TransactionError,
            VersionedTransaction,
        },
    },
    std::{
        pin::Pin,
        result,
        time::Duration,
    },
    time::OffsetDateTime,
    uuid::Uuid,
};

/// The trait for handling the auction for the service.
#[async_trait]
pub trait Auctionable<T: ChainTrait> {
    /// This is the type that is used to trigger the auction submission and conclusion.
    type Trigger: std::fmt::Debug + Clone;
    /// The trigger stream type when subscribing to new triggers on the ws client for the chain.
    type TriggerStream<'a>: Stream<Item = Self::Trigger> + Unpin + Send + 'a;
    /// The ws client type for the chain.
    type WsClient;
    /// The conclusion result type when try to conclude the auction for the chain.
    type ConclusionResult;
    /// The minimum lifetime for an auction. If any bid for auction is older than this, the auction is ready to be submitted.
    const AUCTION_MINIMUM_LIFETIME: Duration;

    /// Get the ws client for the chain.
    async fn get_ws_client(&self) -> Result<Self::WsClient>;
    /// Get the trigger stream for the ws client to subscribe to new triggers.
    async fn get_trigger_stream<'a>(client: &'a Self::WsClient) -> Result<Self::TriggerStream<'a>>;
    /// Check if the auction is ready to be concluded based on the trigger.
    fn is_ready_to_conclude(trigger: Self::Trigger) -> bool;

    /// Get the winner bids for the auction. Sorting bids by bid amount and simulating the bids to determine the winner bids.
    async fn get_winner_bids(
        &self,
        auction: &entities::Auction<T>,
    ) -> Result<Vec<entities::Bid<T>>>;
    /// Submit the bids for the auction on the chain.
    async fn submit_bids(
        &self,
        permission_key: entities::PermissionKey<T>,
        bids: Vec<entities::Bid<T>>,
    ) -> Result<entities::TxHash<T>>;
    /// Get the bid results for the bids submitted for the auction after the transaction is concluded.
    /// Order of the returned BidStatus is as same as the order of the bids.
    async fn get_bid_results(
        &self,
        bids: Vec<entities::Bid<T>>,
        bid_status_auction: entities::BidStatusAuction<T::BidStatusType>,
    ) -> Result<Option<Vec<T::BidStatusType>>>;

    /// Check if the auction winner transaction should be submitted on chain for the permission key.
    async fn get_submission_state(
        &self,
        permission_key: &entities::PermissionKey<T>,
    ) -> entities::SubmitType;

    /// Get the new status for the bid after the bids of the auction are submitted.
    fn get_new_status(
        bid: &entities::Bid<T>,
        submitted_bids: &[entities::Bid<T>],
        bid_status_auction: entities::BidStatusAuction<T::BidStatusType>,
    ) -> T::BidStatusType;
}


// While we are submitting bids together, increasing this number will have the following effects:
// 1. There will be more gas required for the transaction, which will result in a higher minimum bid amount.
// 2. The transaction size limit will be reduced for each bid.
// 3. Gas consumption limit will decrease for the bid
pub const TOTAL_BIDS_PER_AUCTION_EVM: usize = 3;
const EXTRA_GAS_FOR_SUBMISSION: u32 = 500 * 1000;

#[async_trait]
impl Auctionable<Evm> for Service<Evm> {
    type Trigger = Block<H256>;
    type TriggerStream<'a> = SubscriptionStream<'a, Ws, Block<H256>>;
    type WsClient = Provider<Ws>;
    type ConclusionResult = TransactionReceipt;

    const AUCTION_MINIMUM_LIFETIME: Duration = Duration::from_secs(1);

    async fn get_ws_client(&self) -> Result<Self::WsClient> {
        let ws = Ws::connect(self.config.chain_config.ws_address.clone()).await?;
        Ok(Provider::new(ws))
    }

    async fn get_trigger_stream<'a>(client: &'a Self::WsClient) -> Result<Self::TriggerStream<'a>> {
        let block_stream = client.subscribe_blocks().await?;
        Ok(block_stream)
    }

    fn is_ready_to_conclude(_trigger: Self::Trigger) -> bool {
        true
    }

    #[tracing::instrument(skip_all)]
    async fn get_winner_bids(
        &self,
        auction: &entities::Auction<Evm>,
    ) -> Result<Vec<entities::Bid<Evm>>> {
        // TODO How we want to perform simulation, pruning, and determination
        if auction.bids.is_empty() {
            return Ok(vec![]);
        }

        let mut bids = auction.bids.clone();
        bids.sort_by(|a, b| b.amount.cmp(&a.amount));
        let bids: Vec<entities::Bid<Evm>> =
            bids.into_iter().take(TOTAL_BIDS_PER_AUCTION_EVM).collect();
        let simulation_result = self
            .get_simulation_call(
                auction.permission_key.clone(),
                bids.clone()
                    .into_iter()
                    .map(|b| (b, false).into())
                    .collect(),
            )
            .await?;

        match simulation_result
            .iter()
            .position(|status| status.external_success)
        {
            Some(index) => Ok(bids.into_iter().skip(index).collect()),
            None => Ok(vec![]),
        }
    }

    #[tracing::instrument(skip_all)]
    async fn submit_bids(
        &self,
        permission_key: entities::PermissionKey<Evm>,
        bids: Vec<entities::Bid<Evm>>,
    ) -> Result<entities::TxHash<Evm>> {
        let gas_estimate = bids
            .iter()
            .fold(U256::zero(), |sum, b| sum + b.chain_data.gas_limit);
        let tx_hash = self
            .config
            .chain_config
            .express_relay
            .contract
            .multicall(
                permission_key,
                bids.into_iter().map(|b| (b, false).into()).collect(),
            )
            .gas(gas_estimate + EXTRA_GAS_FOR_SUBMISSION)
            .send()
            .await?
            .tx_hash();
        Ok(tx_hash)
    }

    async fn get_bid_results(
        &self,
        bids: Vec<entities::Bid<Evm>>,
        bid_status_auction: entities::BidStatusAuction<entities::BidStatusEvm>,
    ) -> Result<Option<Vec<entities::BidStatusEvm>>> {
        let receipt = self
            .config
            .chain_config
            .provider
            .get_transaction_receipt(bid_status_auction.tx_hash)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to get transaction receipt: {:?}", e))?;
        match receipt {
            Some(receipt) => {
                let decoded_logs = Evm::decode_logs_for_receipt(&receipt);
                Ok(Some(
                    bids.iter()
                        .map(|b| {
                            match decoded_logs
                                .iter()
                                .find(|decoded_log| Uuid::from_bytes(decoded_log.bid_id) == b.id)
                            {
                                Some(decoded_log) => {
                                    match decoded_log.multicall_status.external_success {
                                        true => entities::BidStatusEvm::Won {
                                            index:   decoded_log.multicall_index.as_u32(),
                                            auction: bid_status_auction.clone(),
                                        },
                                        false => entities::BidStatusEvm::Lost {
                                            index:   Some(decoded_log.multicall_index.as_u32()),
                                            auction: Some(bid_status_auction.clone()),
                                        },
                                    }
                                }
                                None => entities::BidStatusEvm::Lost {
                                    auction: Some(bid_status_auction.clone()),
                                    index:   None,
                                },
                            }
                        })
                        .collect(),
                ))
            }
            None => Ok(None),
        }
    }

    async fn get_submission_state(
        &self,
        _permission_key: &entities::PermissionKey<Evm>,
    ) -> entities::SubmitType {
        entities::SubmitType::ByServer
    }

    fn get_new_status(
        bid: &entities::Bid<Evm>,
        submitted_bids: &[entities::Bid<Evm>],
        bid_status_auction: entities::BidStatusAuction<entities::BidStatusEvm>,
    ) -> entities::BidStatusEvm {
        let index = submitted_bids.iter().position(|b| b.id == bid.id);
        match index {
            Some(index) => entities::BidStatusEvm::Submitted {
                auction: bid_status_auction,
                index:   index as u32,
            },
            None => entities::BidStatusEvm::Lost {
                auction: Some(bid_status_auction),
                index:   None,
            },
        }
    }
}

/// This is to make sure we are not missing any transaction.
/// We run this once every minute (150 slots).
const CONCLUSION_TRIGGER_SLOT_INTERVAL_SVM: u64 = 150;
const BID_MAXIMUM_LIFE_TIME_SVM: Duration = Duration::from_secs(120);

#[async_trait]
impl Auctionable<Svm> for Service<Svm> {
    type Trigger = SlotInfo;
    type TriggerStream<'a> = Pin<Box<dyn Stream<Item = Self::Trigger> + Send + 'a>>;
    type WsClient = PubsubClient;
    type ConclusionResult = result::Result<(), TransactionError>;

    const AUCTION_MINIMUM_LIFETIME: Duration = Duration::from_millis(400);

    async fn get_ws_client(&self) -> Result<Self::WsClient> {
        PubsubClient::new(&self.config.chain_config.ws_address)
            .await
            .map_err(|e| {
                tracing::error!(error = ?e, "Error while creating svm pub sub client");
                anyhow::anyhow!(e)
            })
    }

    async fn get_trigger_stream<'a>(client: &'a Self::WsClient) -> Result<Self::TriggerStream<'a>> {
        let (slot_subscribe, _) = client.slot_subscribe().await?;
        Ok(slot_subscribe)
    }

    fn is_ready_to_conclude(trigger: Self::Trigger) -> bool {
        trigger.slot % CONCLUSION_TRIGGER_SLOT_INTERVAL_SVM == 0
    }

    async fn get_winner_bids(
        &self,
        auction: &entities::Auction<Svm>,
    ) -> Result<Vec<entities::Bid<Svm>>> {
        let mut bids = auction.bids.clone();
        bids.sort_by(|a, b| b.amount.cmp(&a.amount));
        for bid in bids.iter() {
            match self
                .simulate_bid(&entities::BidCreate {
                    chain_id:        bid.chain_id.clone(),
                    initiation_time: bid.initiation_time,
                    profile:         None,
                    chain_data:      entities::BidChainDataCreateSvm {
                        transaction: bid.chain_data.transaction.clone(),
                    },
                })
                .await
            {
                Err(RestError::SimulationError {
                    result: _,
                    reason: _,
                }) => {}
                // Either simulation was successful or we can't simulate at this moment
                _ => return Ok(vec![bid.clone()]),
            }
        }
        Ok(vec![])
    }

    #[tracing::instrument(skip_all)]
    async fn submit_bids(
        &self,
        _permission_key: entities::PermissionKey<Svm>,
        bids: Vec<entities::Bid<Svm>>,
    ) -> Result<entities::TxHash<Svm>> {
        if bids.is_empty() {
            return Err(anyhow::anyhow!("No bids to submit"));
        }

        let mut bid = bids[0].clone();
        self.add_relayer_signature(&mut bid);
        match self.send_transaction(&bid.chain_data.transaction).await {
            Ok(response) => Ok(response),
            Err(e) => {
                tracing::error!(error = ?e, "Error while submitting bid");
                Err(anyhow::anyhow!(e))
            }
        }
    }

    async fn get_bid_results(
        &self,
        bids: Vec<entities::Bid<Svm>>,
        bid_status_auction: entities::BidStatusAuction<entities::BidStatusSvm>,
    ) -> Result<Option<Vec<entities::BidStatusSvm>>> {
        if bids.is_empty() {
            return Ok(Some(vec![]));
        }

        if bids.len() != 1 {
            tracing::warn!(bid_status_auction = ?bid_status_auction, bids = ?bids, "multiple bids found for transaction hash");
        }

        //TODO: this can be optimized out if triggered by websocket events
        let status = self
            .config
            .chain_config
            .client
            .get_signature_status_with_commitment(
                &bid_status_auction.tx_hash,
                CommitmentConfig::confirmed(),
            )
            .await?;

        let status = match status {
            Some(res) => match res {
                Ok(()) => entities::BidStatusSvm::Won {
                    auction: bid_status_auction,
                },
                Err(_) => entities::BidStatusSvm::Lost {
                    auction: Some(bid_status_auction),
                },
            },
            None => {
                // not yet confirmed
                // TODO Use the correct version of the expiration algorithm, which is:
                // the tx is not expired as long as the block hash is still recent.
                // Assuming a certain block time, the two minute threshold is good enough but in some cases, it's not correct.
                if bids[0].initiation_time + BID_MAXIMUM_LIFE_TIME_SVM < OffsetDateTime::now_utc() {
                    // If the bid is older than the maximum lifetime, it means that the block hash is now too old and the transaction is expired.
                    entities::BidStatusSvm::Expired {
                        auction: bid_status_auction,
                    }
                } else {
                    return Ok(None);
                }
            }
        };

        Ok(Some(vec![status; bids.len()]))
    }

    async fn get_submission_state(
        &self,
        permission_key: &entities::PermissionKey<Svm>,
    ) -> entities::SubmitType {
        if permission_key.0.starts_with(
            &self
                .config
                .chain_config
                .wallet_program_router_account
                .to_bytes(),
        ) {
            if self
                .opportunity_service
                .get_live_opportunities(GetLiveOpportunitiesInput {
                    key: opportunity::entities::OpportunityKey(
                        self.config.chain_id.clone(),
                        Bytes::from(permission_key.0),
                    ),
                })
                .await
                .is_empty()
            {
                entities::SubmitType::Invalid
            } else {
                entities::SubmitType::ByOther
            }
        } else {
            entities::SubmitType::ByServer
        }
    }

    fn get_new_status(
        bid: &entities::Bid<Svm>,
        submitted_bids: &[entities::Bid<Svm>],
        bid_status_auction: entities::BidStatusAuction<entities::BidStatusSvm>,
    ) -> entities::BidStatusSvm {
        if submitted_bids.iter().any(|b| b.id == bid.id) {
            entities::BidStatusSvm::Submitted {
                auction: bid_status_auction,
            }
        } else {
            entities::BidStatusSvm::Lost {
                auction: Some(bid_status_auction),
            }
        }
    }
}

const SEND_TRANSACTION_RETRY_COUNT_SVM: i32 = 5;

impl Service<Svm> {
    pub fn add_relayer_signature(&self, bid: &mut entities::Bid<Svm>) {
        let relayer = &self.config.chain_config.express_relay.relayer;
        let serialized_message = bid.chain_data.transaction.message.serialize();
        let relayer_signature_pos = bid
            .chain_data
            .transaction
            .message
            .static_account_keys()
            .iter()
            .position(|p| p.eq(&relayer.pubkey()))
            .expect("Relayer not found in static account keys");
        bid.chain_data.transaction.signatures[relayer_signature_pos] =
            relayer.sign_message(&serialized_message);
    }

    async fn send_transaction(
        &self,
        tx: &VersionedTransaction,
    ) -> solana_client::client_error::Result<Signature> {
        let config = RpcSendTransactionConfig {
            skip_preflight: true,
            max_retries: Some(0),
            ..RpcSendTransactionConfig::default()
        };
        let res = self
            .config
            .chain_config
            .tx_broadcaster_client
            .send_transaction_with_config(tx, config)
            .await?;
        let tx_cloned = tx.clone();
        let mut receiver = self.config.chain_config.log_sender.subscribe();
        let signature_bs58 = bs58::encode(res).into_string();
        self.task_tracker.spawn({
            let service = self.clone();
            async move {
                for _ in 0..SEND_TRANSACTION_RETRY_COUNT_SVM {
                    tokio::time::sleep(Duration::from_secs(2)).await;

                    // Do not wait for the logs to be received
                    // just check if the transaction is in the logs already
                    while let Ok(log) = receiver.try_recv() {
                        if log.value.signature.eq(&signature_bs58) {
                            return;
                        }
                    }
                    if let Err(e) = service
                        .config
                        .chain_config
                        .client
                        .send_transaction_with_config(&tx_cloned, config)
                        .await
                    {
                        tracing::error!(error = ?e, "Failed to resend transaction");
                    }
                }
            }
        });
        Ok(res)
    }
}
