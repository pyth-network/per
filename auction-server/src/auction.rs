use {
    crate::{
        api::RestError,
        config::{
            ChainId,
            EthereumConfig,
        },
        models::Auction,
        server::{
            EXIT_CHECK_INTERVAL,
            SHOULD_EXIT,
        },
        state::{
            AuctionLock,
            BidAmount,
            BidStatus,
            ChainStore,
            SimulatedBid,
            Store,
        },
    },
    anyhow::{
        anyhow,
        Result,
    },
    ethers::{
        abi,
        contract::{
            abigen,
            ContractError,
            EthError,
            EthEvent,
            FunctionCall,
        },
        middleware::{
            transformer::{
                Transformer,
                TransformerError,
            },
            NonceManagerMiddleware,
            SignerMiddleware,
            TransformerMiddleware,
        },
        providers::{
            Http,
            Middleware,
            Provider,
            Ws,
        },
        signers::{
            LocalWallet,
            Signer,
        },
        types::{
            transaction::eip2718::TypedTransaction,
            Address,
            BlockNumber,
            Bytes,
            TransactionReceipt,
            TransactionRequest,
            H160,
            H256,
            U256,
        },
    },
    futures::{
        future::join_all,
        StreamExt,
    },
    serde::{
        Deserialize,
        Serialize,
    },
    sqlx::types::time::OffsetDateTime,
    std::{
        result,
        sync::{
            atomic::Ordering,
            Arc,
        },
        time::Duration,
    },
    tokio::sync::MutexGuard,
    utoipa::ToSchema,
    uuid::Uuid,
};

abigen!(
    ExpressRelay,
    "../per_multicall/out/ExpressRelay.sol/ExpressRelay.json"
);
pub type ExpressRelayContract = ExpressRelay<Provider<Http>>;
pub type SignableProvider = TransformerMiddleware<
    NonceManagerMiddleware<SignerMiddleware<Provider<Http>, LocalWallet>>,
    LegacyTxTransformer,
>;
pub type SignableExpressRelayContract = ExpressRelay<SignableProvider>;

impl TryFrom<EthereumConfig> for Provider<Http> {
    type Error = anyhow::Error;
    fn try_from(config: EthereumConfig) -> Result<Self, Self::Error> {
        Provider::<Http>::try_from(config.geth_rpc_addr.clone()).map_err(|err| {
            anyhow!(
                "Failed to connect to {rpc_addr}: {:?}",
                err,
                rpc_addr = config.geth_rpc_addr
            )
        })
    }
}

impl From<([u8; 16], H160, Bytes, U256)> for MulticallData {
    fn from(x: ([u8; 16], H160, Bytes, U256)) -> Self {
        MulticallData {
            bid_id:          x.0,
            target_contract: x.1,
            target_calldata: x.2,
            bid_amount:      x.3,
        }
    }
}

pub fn get_simulation_call(
    relayer: Address,
    provider: Provider<Http>,
    chain_config: EthereumConfig,
    permission_key: Bytes,
    multicall_data: Vec<MulticallData>,
) -> FunctionCall<Arc<Provider<Http>>, Provider<Http>, Vec<MulticallStatus>> {
    let client = Arc::new(provider);
    let express_relay_contract =
        ExpressRelayContract::new(chain_config.express_relay_contract, client);

    express_relay_contract
        .multicall(permission_key, multicall_data)
        .from(relayer)
        .block(BlockNumber::Pending)
}

/// Transformer that converts a transaction into a legacy transaction if use_legacy_tx is true.
#[derive(Clone, Debug)]
pub struct LegacyTxTransformer {
    use_legacy_tx: bool,
}

impl Transformer for LegacyTxTransformer {
    fn transform(&self, tx: &mut TypedTransaction) -> Result<(), TransformerError> {
        if self.use_legacy_tx {
            let legacy_request: TransactionRequest = (*tx).clone().into();
            *tx = legacy_request.into();
            Ok(())
        } else {
            Ok(())
        }
    }
}

pub async fn submit_bids(
    express_relay_contract: Arc<SignableExpressRelayContract>,
    permission: Bytes,
    multicall_data: Vec<MulticallData>,
) -> Result<H256, ContractError<SignableProvider>> {
    let call = express_relay_contract.multicall(permission, multicall_data);
    let mut gas_estimate = call.estimate_gas().await?;

    let gas_multiplier = U256::from(2); //TODO: smarter gas estimation
    gas_estimate *= gas_multiplier;
    let call_with_gas = call.gas(gas_estimate);
    let send_call = call_with_gas.send().await?;

    Ok(send_call.tx_hash())
}

impl From<SimulatedBid> for MulticallData {
    fn from(bid: SimulatedBid) -> Self {
        MulticallData {
            bid_id:          bid.id.into_bytes(),
            target_contract: bid.target_contract,
            target_calldata: bid.target_calldata,
            bid_amount:      bid.bid_amount,
        }
    }
}

async fn get_winner_bids(
    bids: &[SimulatedBid],
    permission_key: Bytes,
    store: Arc<Store>,
    chain_store: &ChainStore,
) -> Result<Vec<SimulatedBid>, ContractError<Provider<Http>>> {
    // TODO How we want to perform simulation, pruning, and determination
    if bids.is_empty() {
        return Ok(vec![]);
    }

    let mut bids = bids.to_owned();
    bids.sort_by(|a, b| b.bid_amount.cmp(&a.bid_amount));

    let simulation_result = get_simulation_call(
        store.relayer.address(),
        chain_store.provider.clone(),
        chain_store.config.clone(),
        permission_key.clone(),
        bids.clone().into_iter().map(|b| b.into()).collect(),
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

fn get_bid_status(decoded_log: &MulticallIssuedFilter, receipt: &TransactionReceipt) -> BidStatus {
    match decoded_log.multicall_status.external_success {
        true => BidStatus::Won {
            index:  decoded_log.multicall_index.as_u32(),
            result: receipt.transaction_hash,
        },
        false => BidStatus::Lost {
            index:  Some(decoded_log.multicall_index.as_u32()),
            result: Some(receipt.transaction_hash),
        },
    }
}

fn decode_logs_for_receipt(receipt: &TransactionReceipt) -> Vec<MulticallIssuedFilter> {
    receipt
        .logs
        .clone()
        .into_iter()
        .filter_map(|log| MulticallIssuedFilter::decode_log(&log.into()).ok())
        .collect()
}


const AUCTION_MINIMUM_LIFETIME: Duration = Duration::from_secs(1);
// An auction is ready if there are any bids with a lifetime of AUCTION_MINIMUM_LIFETIME
fn is_ready_for_auction(bids: Vec<SimulatedBid>, bid_collection_time: OffsetDateTime) -> bool {
    bids.iter()
        .any(|bid| bid_collection_time - bid.initiation_time > AUCTION_MINIMUM_LIFETIME)
}

async fn conclude_submitted_auction(store: Arc<Store>, auction: Auction) -> Result<()> {
    if let Some(tx_hash) = auction.tx_hash {
        let chain_store = store
            .chains
            .get(&auction.chain_id)
            .ok_or(anyhow!("Chain not found: {}", auction.chain_id))?;

        let receipt = chain_store
            .provider
            .get_transaction_receipt(tx_hash)
            .await
            .map_err(|e| anyhow!("Failed to get transaction receipt: {:?}", e))?;

        if let Some(receipt) = receipt {
            let decoded_logs = decode_logs_for_receipt(&receipt);
            let auction = store
                .conclude_auction(auction)
                .await
                .map_err(|e| anyhow!("Failed to conclude auction: {:?}", e))?;
            let bids: Vec<SimulatedBid> = store.bids_for_submitted_auction(auction.clone()).await;

            join_all(decoded_logs.iter().map(|decoded_log| async {
                if let Some(bid) = bids
                    .clone()
                    .into_iter()
                    .find(|b| b.id == Uuid::from_bytes(decoded_log.bid_id))
                {
                    if let Err(err) = store
                        .broadcast_bid_status_and_update(
                            bid,
                            get_bid_status(decoded_log, &receipt),
                            Some(&auction),
                        )
                        .await
                    {
                        tracing::error!("Failed to broadcast bid status: {:?}", err);
                    }
                }
            }))
            .await;
            store.remove_submitted_auction(auction).await;
        }
    }
    Ok(())
}

async fn conclude_submitted_auctions(store: Arc<Store>, chain_id: String) {
    let auctions = store.get_submitted_auctions(&chain_id).await;

    tracing::info!(
        "Chain: {chain_id} Auctions to conclude {auction_len}",
        chain_id = chain_id,
        auction_len = auctions.len()
    );

    for auction in auctions.iter() {
        store
            .task_tracker
            .spawn(conclude_submitted_auction(store.clone(), auction.clone()));
    }
}

async fn submit_auction_for_bids<'a>(
    bids: Vec<SimulatedBid>,
    bid_collection_time: OffsetDateTime,
    permission_key: Bytes,
    chain_id: String,
    store: Arc<Store>,
    chain_store: &ChainStore,
    _auction_mutex_gaurd: MutexGuard<'a, ()>,
) -> Result<()> {
    let bids: Vec<SimulatedBid> = bids
        .into_iter()
        .filter(|bid| bid.status == BidStatus::Pending)
        .collect();

    if bids.is_empty() {
        return Ok(());
    }

    if !is_ready_for_auction(bids.clone(), bid_collection_time) {
        tracing::info!("Auction for {} is not ready yet", permission_key);
        return Ok(());
    }

    let winner_bids =
        get_winner_bids(&bids, permission_key.clone(), store.clone(), chain_store).await?;
    if winner_bids.is_empty() {
        for bid in bids.iter() {
            store
                .broadcast_bid_status_and_update(
                    bid.clone(),
                    BidStatus::Lost {
                        result: None,
                        index:  None,
                    },
                    None,
                )
                .await?;
        }
        return Ok(());
    }

    let mut auction = store
        .init_auction(
            permission_key.clone(),
            chain_id.clone(),
            bid_collection_time,
        )
        .await?;

    tracing::info!(
        "Submission for {} on chain {} started at {}",
        permission_key,
        chain_id,
        OffsetDateTime::now_utc()
    );
    let submit_bids_call = submit_bids(
        chain_store.express_relay_contract.clone(),
        permission_key.clone(),
        winner_bids.clone().into_iter().map(|b| b.into()).collect(),
    );

    match submit_bids_call.await {
        Ok(tx_hash) => {
            tracing::debug!("Submitted transaction: {:?}", tx_hash);
            auction = store.submit_auction(auction, tx_hash).await?;
            join_all(winner_bids.iter().enumerate().map(|(i, bid)| {
                // TODO update the status of bids to lost for those that are not going to be submitted to the chain for this auction
                let (index, store, bid, auction) =
                    (i as u32, store.clone(), bid.clone(), auction.clone());
                async move {
                    store
                        .broadcast_bid_status_and_update(
                            bid,
                            BidStatus::Submitted {
                                result: tx_hash,
                                index,
                            },
                            Some(&auction),
                        )
                        .await
                }
            }))
            .await;
        }
        Err(err) => {
            tracing::error!("Transaction failed to submit: {:?}", err);
        }
    };
    Ok(())
}

async fn submit_auction_for_lock(
    store: Arc<Store>,
    permission_key: Bytes,
    chain_id: String,
    auction_lock: AuctionLock,
) -> Result<()> {
    let acquired_lock = auction_lock.lock().await;
    let chain_store = store
        .chains
        .get(&chain_id)
        .ok_or(anyhow!("Chain not found: {}", chain_id))?;

    let bid_collection_time: OffsetDateTime = OffsetDateTime::now_utc();
    let bids: Vec<SimulatedBid> = store
        .get_bids(&(permission_key.clone(), chain_id.clone()))
        .await;

    submit_auction_for_bids(
        bids,
        bid_collection_time,
        permission_key.clone(),
        chain_id.clone(),
        store.clone(),
        chain_store,
        acquired_lock,
    )
    .await
}

async fn submit_auction(store: Arc<Store>, permission_key: Bytes, chain_id: String) -> Result<()> {
    let key = (permission_key.clone(), chain_id.clone());
    let auction_lock = store.get_auction_lock(key.clone()).await;
    let result =
        submit_auction_for_lock(store.clone(), permission_key, chain_id, auction_lock).await;
    store.remove_auction_lock(&key).await;
    result
}

pub fn get_express_relay_contract(
    address: Address,
    provider: Provider<Http>,
    relayer: LocalWallet,
    use_legacy_tx: bool,
    network_id: u64,
) -> SignableExpressRelayContract {
    let transformer = LegacyTxTransformer { use_legacy_tx };
    let client = Arc::new(TransformerMiddleware::new(
        NonceManagerMiddleware::new(
            SignerMiddleware::new(provider, relayer.clone().with_chain_id(network_id)),
            relayer.address(),
        ),
        transformer,
    ));
    SignableExpressRelayContract::new(address, client)
}

async fn submit_auctions(store: Arc<Store>, chain_id: String) -> Result<()> {
    let permission_keys = store.get_permission_keys_for_auction(&chain_id).await;

    tracing::info!(
        "Chain: {chain_id} Auctions to process {auction_len}",
        chain_id = chain_id,
        auction_len = permission_keys.len()
    );

    for permission_key in permission_keys.iter() {
        store.task_tracker.spawn(submit_auction(
            store.clone(),
            permission_key.clone(),
            chain_id.clone(),
        ));
    }
    Ok(())
}

async fn get_ws_provider(store: Arc<Store>, chain_id: String) -> Result<Provider<Ws>> {
    let chain_store = store
        .chains
        .get(&chain_id)
        .ok_or(anyhow!("Chain not found: {}", chain_id))?;
    let ws = Ws::connect(chain_store.config.geth_ws_addr.clone()).await?;
    Ok(Provider::new(ws))
}

pub async fn run_submission_loop(store: Arc<Store>, chain_id: String) -> Result<()> {
    tracing::info!("Starting transaction submitter...");
    let mut exit_check_interval = tokio::time::interval(EXIT_CHECK_INTERVAL);

    let ws_provider = get_ws_provider(store.clone(), chain_id.clone()).await?;
    let mut stream = ws_provider.subscribe_blocks().await?;

    while !SHOULD_EXIT.load(Ordering::Acquire) {
        tokio::select! {
            block = stream.next() => {
                if block.is_none() {
                    return Err(anyhow!("Block stream ended for chain: {}", chain_id));
                }

                tracing::debug!("New block received for {} at {}: {:?}", chain_id, OffsetDateTime::now_utc(), block);
                store.task_tracker.spawn(
                    submit_auctions(
                        store.clone(),
                        chain_id.clone(),
                    )
                );
                store.task_tracker.spawn(
                    conclude_submitted_auctions(store.clone(), chain_id.clone())
                );
            }
            _ = exit_check_interval.tick() => {}
        }
    }
    tracing::info!("Shutting down transaction submitter...");
    Ok(())
}

#[derive(Serialize, Deserialize, ToSchema, Clone)]
pub struct Bid {
    /// The permission key to bid on.
    #[schema(example = "0xdeadbeef", value_type = String)]
    pub permission_key:  Bytes,
    /// The chain id to bid on.
    #[schema(example = "op_sepolia", value_type = String)]
    pub chain_id:        ChainId,
    /// The contract address to call.
    #[schema(example = "0xcA11bde05977b3631167028862bE2a173976CA11", value_type = String)]
    pub target_contract: abi::Address,
    /// Calldata for the contract call.
    #[schema(example = "0xdeadbeef", value_type = String)]
    pub target_calldata: Bytes,
    /// Amount of bid in wei.
    #[schema(example = "10", value_type = String)]
    #[serde(with = "crate::serde::u256")]
    pub amount:          BidAmount,
}

pub async fn handle_bid(
    store: Arc<Store>,
    bid: Bid,
    initiation_time: OffsetDateTime,
) -> result::Result<Uuid, RestError> {
    let chain_store = store
        .chains
        .get(&bid.chain_id)
        .ok_or(RestError::InvalidChainId)?;
    let call = get_simulation_call(
        store.relayer.address(),
        chain_store.provider.clone(),
        chain_store.config.clone(),
        bid.permission_key.clone(),
        vec![MulticallData::from((
            Uuid::new_v4().into_bytes(),
            bid.target_contract,
            bid.target_calldata.clone(),
            bid.amount,
        ))],
    );

    match call.await {
        Ok(results) => {
            if !results[0].external_success {
                return Err(RestError::SimulationError {
                    result: results[0].external_result.clone(),
                    reason: results[0].multicall_revert_reason.clone(),
                });
            }
        }
        Err(e) => {
            return match e {
                ContractError::Revert(reason) => Err(RestError::BadParameters(format!(
                    "Contract Revert Error: {}",
                    String::decode_with_selector(&reason)
                        .unwrap_or("unable to decode revert".to_string())
                ))),
                ContractError::MiddlewareError { e: _ } => Err(RestError::TemporarilyUnavailable),
                ContractError::ProviderError { e: _ } => Err(RestError::TemporarilyUnavailable),
                _ => Err(RestError::BadParameters(format!("Error: {}", e))),
            };
        }
    }

    let bid_id = Uuid::new_v4();
    let simulated_bid = SimulatedBid {
        target_contract: bid.target_contract,
        target_calldata: bid.target_calldata.clone(),
        bid_amount: bid.amount,
        id: bid_id,
        permission_key: bid.permission_key.clone(),
        chain_id: bid.chain_id.clone(),
        status: BidStatus::Pending,
        initiation_time,
    };
    store.add_bid(simulated_bid).await?;
    Ok(bid_id)
}
