use {
    super::{
        ChainTrait,
        Service,
    },
    crate::{
        auction::entities,
        kernel::{
            contracts::MulticallIssuedFilter,
            entities::{
                Evm,
                Svm,
            },
        },
        opportunity::{
            self,
            service::get_live_opportunities::GetLiveOpportunitiesInput,
        },
    },
    anyhow::Result,
    axum::async_trait,
    ethers::{
        contract::EthEvent,
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
    futures::{
        future::join_all,
        Stream,
    },
    solana_client::{
        nonblocking::pubsub_client::PubsubClient,
        rpc_config::RpcSendTransactionConfig,
    },
    solana_sdk::{
        commitment_config::CommitmentConfig,
        signature::{
            Signature,
            Signer,
        },
        transaction::TransactionError,
    },
    std::{
        fmt::Debug,
        pin::Pin,
        result,
        task::{
            Context,
            Poll,
        },
        time::Duration,
    },
    time::OffsetDateTime,
    tokio::time::{
        interval,
        Interval,
    },
    uuid::Uuid,
};

/// The trait for handling the auction for the service.
#[async_trait]
pub trait AuctionManager<T: ChainTrait> {
    /// This is the type that is used to trigger the auction submission and conclusion.
    type Trigger: Debug + Clone;
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
    /// Returns None for each bid if the bid is not yet confirmed on chain.
    async fn get_bid_results(
        &self,
        bids: Vec<entities::Bid<T>>,
        bid_status_auction: entities::BidStatusAuction<T::BidStatusType>,
    ) -> Result<Vec<Option<T::BidStatusType>>>;

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
impl AuctionManager<Evm> for Service<Evm> {
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

    #[tracing::instrument(skip_all, fields(auction_id, bid_ids, simulation_result))]
    async fn get_winner_bids(
        &self,
        auction: &entities::Auction<Evm>,
    ) -> Result<Vec<entities::Bid<Evm>>> {
        tracing::Span::current().record("auction_id", auction.id.to_string());

        // TODO How we want to perform simulation, pruning, and determination
        if auction.bids.is_empty() {
            return Ok(vec![]);
        }

        let mut bids = auction.bids.clone();
        tracing::Span::current().record(
            "bid_ids",
            tracing::field::display(entities::BidContainerTracing(&bids)),
        );
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

        tracing::Span::current().record("simulation_result", format!("{:?}", simulation_result));

        match simulation_result
            .iter()
            .position(|status| status.external_success)
        {
            Some(index) => Ok(bids.into_iter().skip(index).collect()),
            None => Ok(vec![]),
        }
    }

    #[tracing::instrument(skip_all, fields(tx_hash))]
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
        tracing::Span::current().record("tx_hash", format!("{:?}", tx_hash));
        Ok(tx_hash)
    }

    #[tracing::instrument(skip_all, fields(bid_ids, tx_hash, auction_id, result))]
    async fn get_bid_results(
        &self,
        bids: Vec<entities::Bid<Evm>>,
        bid_status_auction: entities::BidStatusAuction<entities::BidStatusEvm>,
    ) -> Result<Vec<Option<entities::BidStatusEvm>>> {
        tracing::Span::current().record(
            "bid_ids",
            tracing::field::display(entities::BidContainerTracing(&bids)),
        );
        tracing::Span::current().record("tx_hash", format!("{:?}", bid_status_auction.tx_hash));
        tracing::Span::current().record("auction_id", bid_status_auction.id.to_string());

        let receipt = self
            .config
            .chain_config
            .provider
            .get_transaction_receipt(bid_status_auction.tx_hash)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to get transaction receipt: {:?}", e))?;

        match receipt {
            Some(receipt) => {
                let decoded_logs = Self::decode_logs_for_receipt(&receipt);
                tracing::Span::current().record("result", format!("{:?}", decoded_logs));
                Ok(bids
                    .iter()
                    .map(|b| {
                        Some(
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
                                        false =>
                                        // TODO: add BidStatusEvm::Failed for when the bid gets submitted but fails on-chain
                                        {
                                            entities::BidStatusEvm::Lost {
                                                index:   Some(decoded_log.multicall_index.as_u32()),
                                                auction: Some(bid_status_auction.clone()),
                                            }
                                        }
                                    }
                                }
                                None => entities::BidStatusEvm::Lost {
                                    auction: Some(bid_status_auction.clone()),
                                    index:   None,
                                },
                            },
                        )
                    })
                    .collect())
            }
            None => Ok(vec![None; bids.len()]),
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
/// We run this once every minute (150 * 0.4).
const CONCLUSION_TRIGGER_INTERVAL_SVM: u64 = 150;
const BID_MAXIMUM_LIFE_TIME_SVM: Duration = Duration::from_secs(120);
const TRIGGER_DURATION_SVM: Duration = Duration::from_millis(400);

pub struct TriggerStreamSvm {
    number:   u64,
    interval: Interval,
}

impl TriggerStreamSvm {
    fn new(interval: Interval) -> Self {
        Self {
            number: 0,
            interval,
        }
    }
}

impl Stream for TriggerStreamSvm {
    type Item = u64;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.interval.poll_tick(cx) {
            Poll::Ready(_) => {
                self.number += 1;
                Poll::Ready(Some(self.number))
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

#[async_trait]
impl AuctionManager<Svm> for Service<Svm> {
    type Trigger = u64;
    type TriggerStream<'a> = TriggerStreamSvm;
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

    async fn get_trigger_stream<'a>(
        _client: &'a Self::WsClient,
    ) -> Result<Self::TriggerStream<'a>> {
        Ok(TriggerStreamSvm::new(interval(TRIGGER_DURATION_SVM)))
    }

    fn is_ready_to_conclude(trigger: Self::Trigger) -> bool {
        // To make sure we run it once at the beginning
        trigger % CONCLUSION_TRIGGER_INTERVAL_SVM == 1
    }

    #[tracing::instrument(skip_all, fields(auction_id, bid_ids))]
    async fn get_winner_bids(
        &self,
        auction: &entities::Auction<Svm>,
    ) -> Result<Vec<entities::Bid<Svm>>> {
        tracing::Span::current().record("auction_id", auction.id.to_string());
        tracing::Span::current().record(
            "bid_ids",
            tracing::field::display(entities::BidContainerTracing(&auction.bids)),
        );
        let mut bids = auction.bids.clone();
        bids.sort_by(|a, b| b.amount.cmp(&a.amount));
        return Ok(self
            .config
            .chain_config
            .simulator
            .optimize_bids(&bids)
            .await
            .map(|x| x.value)
            // If the optimization fails (mainly because of rpc issues)
            // we just submit the first bid
            .unwrap_or(bids.first().cloned().map(|b| vec![b]).unwrap_or_default()));
    }

    /// Submit all the svm bids as separate transactions concurrently
    /// Returns Ok if at least one of the transactions is successful
    /// Returns Err if all transactions are failed
    #[tracing::instrument(skip_all, fields(tx_hash))]
    async fn submit_bids(
        &self,
        _permission_key: entities::PermissionKey<Svm>,
        bids: Vec<entities::Bid<Svm>>,
    ) -> Result<entities::TxHash<Svm>> {
        if bids.is_empty() {
            return Err(anyhow::anyhow!("No bids to submit"));
        }

        let send_futures: Vec<_> = bids
            .into_iter()
            .map(|mut bid| {
                self.add_relayer_signature(&mut bid);
                async move { self.send_transaction(&bid).await }
            })
            .collect();

        let results = join_all(send_futures).await;
        let mut result = None;
        for res in results.into_iter() {
            match res {
                Ok(sig) => {
                    result = Some(Ok(sig));
                }
                Err(e) => {
                    tracing::error!(error = ?e, "Error while submitting bid");
                    if result.is_none() {
                        result = Some(Err(anyhow::anyhow!(e)));
                    }
                }
            }
        }
        result.expect("results should not be empty because bids is not empty")
    }

    #[tracing::instrument(skip_all, fields(bid_ids, tx_hash, auction_id, bid_statuses))]
    async fn get_bid_results(
        &self,
        bids: Vec<entities::Bid<Svm>>,
        bid_status_auction: entities::BidStatusAuction<entities::BidStatusSvm>,
    ) -> Result<Vec<Option<entities::BidStatusSvm>>> {
        tracing::Span::current().record(
            "bid_ids",
            tracing::field::display(entities::BidContainerTracing(&bids)),
        );
        tracing::Span::current().record("tx_hash", bid_status_auction.tx_hash.to_string());
        tracing::Span::current().record("auction_id", bid_status_auction.id.to_string());
        if bids.is_empty() {
            return Ok(vec![]);
        }

        //TODO: this can be optimized out if triggered by websocket events
        let signatures: Vec<_> = bids
            .iter()
            .map(|bid| {
                *bid.chain_data
                    .transaction
                    .signatures
                    .first()
                    .expect("Signature array is empty on svm bid tx")
            })
            .collect();
        let statuses: Vec<_> = self
            .config
            .chain_config
            .client
            .get_signature_statuses(&signatures)
            .await?
            .value
            .into_iter()
            .map(|status| {
                status.filter(|status| status.satisfies_commitment(CommitmentConfig::confirmed()))
            })
            .collect();

        tracing::Span::current().record("bid_statuses", format!("{:?}", statuses));
        // TODO: find a better place to put this
        // Remove the pending transactions from the simulator
        join_all(
            statuses
                .iter()
                .zip(signatures.iter())
                .filter_map(|(status, sig)| {
                    status.as_ref().map(|_| {
                        self.config
                            .chain_config
                            .simulator
                            .remove_pending_transaction(sig)
                    })
                }),
        )
        .await;

        let res = statuses
            .iter()
            .zip(bids.iter())
            .map(|(status, bid)| {
                let bid_status_auction = bid_status_auction.clone();

                match status {
                    Some(res) => Some(match res.err {
                        Some(_) => entities::BidStatusSvm::Failed {
                            auction: bid_status_auction,
                        },
                        None => entities::BidStatusSvm::Won {
                            auction: bid_status_auction,
                        },
                    }),
                    None => {
                        // not yet confirmed
                        // TODO Use the correct version of the expiration algorithm, which is:
                        // the tx is not expired as long as the block hash is still recent.
                        // Assuming a certain block time, the two minute threshold is good enough but in some cases, it's not correct.
                        if bid.initiation_time + BID_MAXIMUM_LIFE_TIME_SVM
                            < OffsetDateTime::now_utc()
                        {
                            // If the bid is older than the maximum lifetime, it means that the block hash is now too old and the transaction is expired.
                            Some(entities::BidStatusSvm::Expired {
                                auction: bid_status_auction,
                            })
                        } else {
                            None
                        }
                    }
                }
            })
            .collect();


        Ok(res)
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

const SEND_TRANSACTION_RETRY_COUNT_SVM: i32 = 30;

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

    #[tracing::instrument(skip_all, fields(bid_id, total_tries, tx_hash))]
    async fn blocking_send_transaction(&self, bid: entities::Bid<Svm>, signature: Signature) {
        tracing::Span::current().record("bid_id", bid.id.to_string());
        tracing::Span::current().record("tx_hash", signature.to_string());
        let config = RpcSendTransactionConfig {
            skip_preflight: true,
            max_retries: Some(0),
            ..RpcSendTransactionConfig::default()
        };
        let mut receiver = self.config.chain_config.log_sender.subscribe();
        for retry_count in 0..SEND_TRANSACTION_RETRY_COUNT_SVM {
            tokio::time::sleep(Duration::from_secs(2)).await;

            // Do not wait for the logs to be received
            // just check if the transaction is in the logs already
            while let Ok(log) = receiver.try_recv() {
                if log.value.signature.eq(&signature.to_string()) {
                    tracing::Span::current().record("total_tries", retry_count + 1);
                    return;
                }
            }
            if let Err(e) = self
                .config
                .chain_config
                .client
                .send_transaction_with_config(&bid.chain_data.transaction, config)
                .await
            {
                tracing::error!(error = ?e, "Failed to resend transaction");
            }
        }

        tracing::Span::current().record("total_tries", SEND_TRANSACTION_RETRY_COUNT_SVM + 1);
    }

    #[tracing::instrument(skip_all, fields(bid_id))]
    async fn send_transaction(
        &self,
        bid: &entities::Bid<Svm>,
    ) -> solana_client::client_error::Result<Signature> {
        tracing::Span::current().record("bid_id", bid.id.to_string());
        let tx = &bid.chain_data.transaction;
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
        self.config
            .chain_config
            .simulator
            .add_pending_transaction(tx)
            .await;
        self.task_tracker.spawn({
            let (service, bid) = (self.clone(), bid.clone());
            async move {
                service.blocking_send_transaction(bid, res).await;
            }
        });
        Ok(res)
    }
}

impl Service<Evm> {
    fn decode_logs_for_receipt(receipt: &TransactionReceipt) -> Vec<MulticallIssuedFilter> {
        receipt
            .logs
            .clone()
            .into_iter()
            .filter_map(|log| MulticallIssuedFilter::decode_log(&log.into()).ok())
            .collect()
    }
}
