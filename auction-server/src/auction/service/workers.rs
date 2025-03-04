use {
    super::{
        auction_manager::AuctionManager,
        ChainTrait,
        Service,
    },
    crate::{
        api::ws::UpdateEvent,
        auction::{
            entities,
            service::conclude_auction::ConcludeAuctionWithStatusesInput,
        },
        kernel::entities::{
            Evm,
            Svm,
        },
        server::{
            EXIT_CHECK_INTERVAL,
            SHOULD_EXIT,
        },
    },
    anyhow::{
        anyhow,
        Result,
    },
    axum_prometheus::metrics,
    ethers::providers::Middleware,
    express_relay_api_types::SvmChainUpdate,
    futures::future::join_all,
    solana_client::{
        rpc_config::{
            RpcTransactionLogsConfig,
            RpcTransactionLogsFilter,
        },
        rpc_response::RpcLogsResponse,
    },
    solana_sdk::{
        commitment_config::CommitmentConfig,
        signature::Signature,
    },
    std::{
        str::FromStr,
        sync::atomic::Ordering,
        time::Duration,
    },
    time::OffsetDateTime,
    tokio_stream::StreamExt,
};

impl<T: ChainTrait> Service<T>
where
    Service<T>: AuctionManager<T>,
{
    pub async fn run_submission_loop(&self) -> Result<()> {
        tracing::info!(
            chain_id = self.config.chain_id,
            "Starting transaction submitter..."
        );
        let mut exit_check_interval = tokio::time::interval(EXIT_CHECK_INTERVAL);

        let ws_client = self.get_ws_client().await?;
        let mut stream = Service::get_trigger_stream(&ws_client).await?;

        while !SHOULD_EXIT.load(Ordering::Acquire) {
            tokio::select! {
                trigger = stream.next() => {
                    let trigger = trigger.ok_or(anyhow!("Trigger stream ended for chain: {}", self.config.chain_id))?;
                    tracing::debug!(chain_id = self.config.chain_id, time = ?OffsetDateTime::now_utc(), trigger = ?trigger, "New trigger received");
                    self.task_tracker.spawn({
                        let service = self.clone();
                        async move {
                            service.handle_auctions().await;
                        }
                    });
                }
                _ = exit_check_interval.tick() => {}
            }
        }
        tracing::info!("Shutting down transaction submitter...");
        Ok(())
    }
}

impl Service<Evm> {
    pub async fn run_tracker_loop(&self) -> Result<()> {
        tracing::info!(chain_id = self.config.chain_id, "Starting tracker...");

        let mut exit_check_interval = tokio::time::interval(EXIT_CHECK_INTERVAL);

        // this should be replaced by a subscription to the chain and trigger on new blocks
        let mut submission_interval = tokio::time::interval(Duration::from_secs(10));
        let relayer_address = self
            .config
            .chain_config
            .express_relay
            .contract
            .get_relayer_address();
        while !SHOULD_EXIT.load(Ordering::Acquire) {
            tokio::select! {
                _ = submission_interval.tick() => {
                    match self.config.chain_config.provider.get_balance(relayer_address, None).await {
                        Ok(r) => {
                            // This conversion to u128 is fine as the total balance will never cross the limits
                            // of u128 practically.
                            // The f64 conversion is made to be able to serve metrics within the constraints of Prometheus.
                            // The balance is in wei, so we need to divide by 1e18 to convert it to eth.
                            let balance = r.as_u128() as f64 / 1e18;
                            let label = [
                                ("chain_id", self.config.chain_id.clone()),
                                ("address", format!("{:?}", relayer_address)),
                            ];
                            metrics::gauge!("relayer_balance", &label).set(balance);
                        }
                        Err(e) => {
                            tracing::error!("Error while getting balance. error: {:?}", e);
                        }
                    };
                }
                _ = exit_check_interval.tick() => {
                }
            }
        }
        tracing::info!("Shutting down tracker...");
        Ok(())
    }
}

const GET_LATEST_BLOCKHASH_INTERVAL_SVM: Duration = Duration::from_secs(5);
impl Service<Svm> {
    pub async fn conclude_auction_for_log(
        &self,
        auction: entities::Auction<Svm>,
        log: RpcLogsResponse,
    ) -> Result<()> {
        let signature = Signature::from_str(&log.signature)?;
        if let Some(bid) = auction
            .bids
            .iter()
            .find(|bid| bid.chain_data.transaction.signatures[0] == signature)
        {
            let bid_status = match log.err {
                Some(_) => entities::BidStatusSvm::Failed {
                    auction: entities::BidStatusAuction {
                        id:      auction.id,
                        tx_hash: signature,
                    },
                },
                None => entities::BidStatusSvm::Won {
                    auction: entities::BidStatusAuction {
                        id:      auction.id,
                        tx_hash: signature,
                    },
                },
            };

            self.conclude_auction_with_statuses(ConcludeAuctionWithStatusesInput {
                auction:      auction.clone(),
                bid_statuses: vec![(bid_status, bid.clone())],
            })
            .await
            .map_err(|e| {
                tracing::error!(
                    error = ?e,
                    auction_id = ?auction.id,
                    tx_hash = ?signature,
                    "Failed to conclude auction with statuses"
                );
                e
            })?;
        }
        Ok(())
    }

    pub async fn run_auction_conclusion_loop(&self) -> Result<()> {
        tracing::info!(
            chain_id = self.config.chain_id,
            "Starting auction conclusion..."
        );
        let mut exit_check_interval = tokio::time::interval(EXIT_CHECK_INTERVAL);
        let mut stream = self.config.chain_config.log_sender.subscribe();
        while !SHOULD_EXIT.load(Ordering::Acquire) {
            tokio::select! {
                rpc_log = stream.recv() => {
                    match rpc_log {
                        Err(err) => return Err(anyhow!("Error while receiving log trigger for chain {}: {:?}", self.config.chain_id, err)),
                        Ok(rpc_log) => {
                            tracing::debug!(
                                chain_id = self.config.chain_id,
                                time = ?OffsetDateTime::now_utc(),
                                log = ?rpc_log.clone(),
                                "New log trigger received",
                            );
                            if let Ok(signature) = Signature::from_str(&rpc_log.value.signature) {
                                self.task_tracker.spawn({
                                    let service = self.clone();
                                    async move {
                                        let in_memory_auctions = service.repo.get_in_memory_auctions().await;
                                        let auctions = in_memory_auctions.iter().filter(|auction| {
                                            auction.bids.iter().any(|bid| {
                                                bid.chain_data.transaction.signatures[0] == signature
                                            })
                                        });
                                        join_all(
                                            auctions.map(|auction| service.conclude_auction_for_log(auction.clone(), rpc_log.value.clone()))
                                        ).await;
                                    }
                                });
                            }
                        }
                    }
                }
                _ = exit_check_interval.tick() => {}
            }
        }
        tracing::info!("Shutting down log listener svm...");

        Ok(())
    }

    pub async fn run_log_listener_loop(&self) -> Result<()> {
        let chain_id = self.config.chain_id.clone();
        tracing::info!(chain_id = chain_id, "Starting log listener...");
        let ws_client = self.get_ws_client().await?;
        let (mut stream, _) = ws_client
            .logs_subscribe(
                RpcTransactionLogsFilter::Mentions(vec![self
                    .config
                    .chain_config
                    .express_relay
                    .program_id
                    .to_string()]),
                RpcTransactionLogsConfig {
                    commitment: Some(CommitmentConfig::confirmed()),
                },
            )
            .await
            .unwrap();
        let mut exit_check_interval = tokio::time::interval(EXIT_CHECK_INTERVAL);
        while !SHOULD_EXIT.load(Ordering::Acquire) {
            tokio::select! {
                rpc_log = stream.next() => {
                    match rpc_log {
                        None => return Err(anyhow!("Log trigger stream ended for chain: {}", &chain_id)),
                        Some(rpc_log) => {
                            tracing::debug!("New log trigger received for {} at {}: {:?}", &chain_id, OffsetDateTime::now_utc(), rpc_log.clone());
                                if let Err(e) = self.config.chain_config.log_sender.send(rpc_log) {
                                    tracing::error!(error = ?e, "Failed to send log to channel");
                                }
                        }
                    };
                }
                _ = exit_check_interval.tick() => {}
            }
        }
        tracing::info!("Shutting down log listener svm...");
        Ok(())
    }

    pub async fn update_metrics(&self) {
        self.repo.update_metrics().await;
    }

    pub async fn run_watcher_loop(&self) -> Result<()> {
        while !SHOULD_EXIT.load(Ordering::Acquire) {
            let responses = (
                self.config
                    .chain_config
                    .client
                    .get_latest_blockhash_with_commitment(CommitmentConfig::finalized())
                    .await,
                self.update_recent_prioritization_fee().await,
            );

            match responses {
                (Ok(block_hash_result), Ok(fee)) => {
                    // TODO we should not know about the api layer here
                    if let Err(e) =
                        self.event_sender
                            .send(UpdateEvent::SvmChainUpdate(SvmChainUpdate {
                                chain_id:                  self.config.chain_id.clone(),
                                blockhash:                 block_hash_result.0,
                                latest_prioritization_fee: fee,
                            }))
                    {
                        tracing::error!("Failed to send chain update: {}", e)
                    };
                }
                (Err(e), _) => {
                    return Err(anyhow!(
                        "Polling blockhash failed for chain {} with error: {}",
                        self.config.chain_id.clone(),
                        e
                    ));
                }
                (_, Err(e)) => {
                    return Err(anyhow!(
                        "Polling prioritization fees failed for chain {} with error: {:?}",
                        self.config.chain_id.clone(),
                        e
                    ));
                }
            }

            tokio::time::sleep(GET_LATEST_BLOCKHASH_INTERVAL_SVM).await;
        }
        Ok(())
    }
}
