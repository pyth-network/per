use {
    crate::{
        api::{
            Auth,
            RestError,
        },
        config::{
            ChainId,
            ConfigEvm,
        },
        models,
        server::{
            EXIT_CHECK_INTERVAL,
            SHOULD_EXIT,
        },
        state::{
            AuctionLock,
            BidAmount,
            BidStatus,
            ChainStoreEvm,
            ChainStoreSvm,
            ExpressRelaySvm,
            PermissionKey,
            SimulatedBid,
            SimulatedBidCoreFields,
            SimulatedBidEvm,
            SimulatedBidSvm,
            Store,
        },
        traced_client::TracedClient,
    },
    ::express_relay::{
        self as express_relay_svm,
    },
    anchor_lang::{
        AnchorDeserialize,
        Discriminator,
    },
    anyhow::{
        anyhow,
        Result,
    },
    axum_prometheus::metrics,
    ethers::{
        abi,
        contract::{
            abigen,
            ContractError,
            ContractRevert,
            EthEvent,
            FunctionCall,
        },
        middleware::{
            gas_oracle::GasOracleMiddleware,
            transformer::{
                Transformer,
                TransformerError,
            },
            GasOracle,
            NonceManagerMiddleware,
            SignerMiddleware,
            TransformerMiddleware,
        },
        providers::{
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
    gas_oracle::EthProviderOracle,
    serde::{
        Deserialize,
        Deserializer,
        Serialize,
    },
    solana_sdk::{
        instruction::CompiledInstruction,
        pubkey::Pubkey,
        signature::Signer as SvmSigner,
        transaction::VersionedTransaction,
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
    "../contracts/evm/out/ExpressRelay.sol/ExpressRelay.json"
);
pub type ExpressRelayContract = ExpressRelay<Provider<TracedClient>>;
pub type SignableProvider = TransformerMiddleware<
    GasOracleMiddleware<
        NonceManagerMiddleware<SignerMiddleware<Provider<TracedClient>, LocalWallet>>,
        EthProviderOracle<Provider<TracedClient>>,
    >,
    LegacyTxTransformer,
>;
pub type SignableExpressRelayContract = ExpressRelay<SignableProvider>;

impl From<([u8; 16], H160, Bytes, U256, U256, bool)> for MulticallData {
    fn from(x: ([u8; 16], H160, Bytes, U256, U256, bool)) -> Self {
        MulticallData {
            bid_id:            x.0,
            target_contract:   x.1,
            target_calldata:   x.2,
            bid_amount:        x.3,
            gas_limit:         x.4,
            revert_on_failure: x.5,
        }
    }
}

const EXTRA_GAS_FOR_SUBMISSION: u32 = 500 * 1000;

pub fn get_simulation_call(
    relayer: Address,
    provider: Provider<TracedClient>,
    chain_config: ConfigEvm,
    permission_key: Bytes,
    multicall_data: Vec<MulticallData>,
) -> FunctionCall<Arc<Provider<TracedClient>>, Provider<TracedClient>, Vec<MulticallStatus>> {
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

#[tracing::instrument(skip_all)]
pub async fn submit_bids(
    express_relay_contract: Arc<SignableExpressRelayContract>,
    permission: Bytes,
    bids: Vec<SimulatedBidEvm>,
) -> Result<H256, ContractError<SignableProvider>> {
    let gas_estimate = bids.iter().fold(U256::zero(), |sum, b| sum + b.gas_limit);
    let tx_hash = express_relay_contract
        .multicall(
            permission,
            bids.into_iter().map(|b| (b, false).into()).collect(),
        )
        .gas(gas_estimate + EXTRA_GAS_FOR_SUBMISSION)
        .send()
        .await?
        .tx_hash();
    Ok(tx_hash)
}

impl From<(SimulatedBidEvm, bool)> for MulticallData {
    fn from((bid, revert_on_failure): (SimulatedBidEvm, bool)) -> Self {
        MulticallData {
            bid_id: bid.core_fields.id.into_bytes(),
            target_contract: bid.target_contract,
            target_calldata: bid.target_calldata,
            bid_amount: bid.core_fields.bid_amount,
            gas_limit: bid.gas_limit,
            revert_on_failure,
        }
    }
}

// While we are submitting bids together, increasing this number will have the following effects:
// 1. There will be more gas required for the transaction, which will result in a higher minimum bid amount.
// 2. The transaction size limit will be reduced for each bid.
// 3. Gas consumption limit will decrease for the bid
const TOTAL_BIDS_PER_AUCTION: usize = 3;

#[tracing::instrument(skip_all)]
async fn get_winner_bids(
    bids: &[SimulatedBidEvm],
    permission_key: Bytes,
    store: Arc<Store>,
    chain_store: &ChainStoreEvm,
) -> Result<Vec<SimulatedBidEvm>, ContractError<Provider<TracedClient>>> {
    // TODO How we want to perform simulation, pruning, and determination
    if bids.is_empty() {
        return Ok(vec![]);
    }

    let mut bids = bids.to_owned();
    bids.sort_by(|a, b| b.core_fields.bid_amount.cmp(&a.core_fields.bid_amount));
    let bids: Vec<SimulatedBidEvm> = bids.into_iter().take(TOTAL_BIDS_PER_AUCTION).collect();

    let simulation_result = get_simulation_call(
        store.relayer.address(),
        chain_store.provider.clone(),
        chain_store.config.clone(),
        permission_key.clone(),
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
        Some(index) => Ok(bids.into_iter().skip(index).take(5).collect()),
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
fn is_ready_for_auction(bids: Vec<SimulatedBidEvm>, bid_collection_time: OffsetDateTime) -> bool {
    bids.iter()
        .any(|bid| bid_collection_time - bid.core_fields.initiation_time > AUCTION_MINIMUM_LIFETIME)
}

async fn conclude_submitted_auction(store: Arc<Store>, auction: models::Auction) -> Result<()> {
    if let Some(tx_hash) = auction.tx_hash.0 {
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
                    .find(|b| b.get_core_fields().id == Uuid::from_bytes(decoded_log.bid_id))
                {
                    if let Err(err) = store
                        .broadcast_bid_status_and_update(
                            bid.clone(),
                            get_bid_status(decoded_log, &receipt),
                            Some(&auction),
                        )
                        .await
                    {
                        tracing::error!(
                            "Failed to broadcast bid status: {:?} - bid: {:?}",
                            err,
                            bid
                        );
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
        store.task_tracker.spawn({
            let (store, auction) = (store.clone(), auction.clone());
            async move {
                if let Err(err) = conclude_submitted_auction(store.clone(), auction.clone()).await {
                    tracing::error!(
                        "Failed to submit auction: {:?} - auction: {:?}",
                        err,
                        auction
                    );
                }
            }
        });
    }
}

async fn broadcast_submitted_bids(
    store: Arc<Store>,
    bids: Vec<SimulatedBid>,
    tx_hash: H256,
    auction: models::Auction,
) {
    join_all(bids.iter().enumerate().map(|(i, bid)| {
        let (store, auction, index) = (store.clone(), auction.clone(), i as u32);
        async move {
            if let Err(err) = store
                .broadcast_bid_status_and_update(
                    bid.to_owned(),
                    BidStatus::Submitted {
                        result: tx_hash,
                        index,
                    },
                    Some(&auction),
                )
                .await
            {
                tracing::error!("Failed to broadcast bid status: {:?} - bid: {:?}", err, bid);
            }
        }
    }))
    .await;
}

async fn broadcast_lost_bids(
    store: Arc<Store>,
    bids: Vec<SimulatedBid>,
    submitted_bids: Vec<SimulatedBid>,
    tx_hash: Option<H256>,
    auction: Option<&models::Auction>,
) {
    join_all(bids.iter().filter_map(|bid| {
        if submitted_bids
            .iter()
            .any(|submitted_bid| bid.get_core_fields().id == submitted_bid.get_core_fields().id)
        {
            return None;
        }

        let store = store.clone();
        Some(async move {
            if let Err(err) = store
                .broadcast_bid_status_and_update(
                    bid.clone(),
                    BidStatus::Lost {
                        result: tx_hash,
                        index:  None,
                    },
                    auction,
                )
                .await
            {
                tracing::error!("Failed to broadcast bid status: {:?} - bid: {:?}", err, bid);
            }
        })
    }))
    .await;
}

async fn submit_auction_for_bids<'a>(
    bids: Vec<SimulatedBidEvm>,
    bid_collection_time: OffsetDateTime,
    permission_key: Bytes,
    chain_id: String,
    store: Arc<Store>,
    chain_store: &ChainStoreEvm,
    _auction_mutex_gaurd: MutexGuard<'a, ()>,
) -> Result<()> {
    let bids: Vec<SimulatedBidEvm> = bids
        .into_iter()
        .filter(|bid| bid.core_fields.status == BidStatus::Pending)
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
        broadcast_lost_bids(
            store.clone(),
            bids.into_iter().map(|b| b.into()).collect(),
            winner_bids.into_iter().map(|b| b.into()).collect(),
            None,
            None,
        )
        .await;
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
        winner_bids.clone(),
    );

    match submit_bids_call.await {
        Ok(tx_hash) => {
            tracing::debug!("Submitted transaction: {:?}", tx_hash);
            auction = store.submit_auction(auction, tx_hash).await?;
            tokio::join!(
                broadcast_submitted_bids(
                    store.clone(),
                    winner_bids.clone().into_iter().map(|b| b.into()).collect(),
                    tx_hash,
                    auction.clone()
                ),
                broadcast_lost_bids(
                    store.clone(),
                    bids.into_iter().map(|b| b.into()).collect(),
                    winner_bids.into_iter().map(|b| b.into()).collect(),
                    Some(tx_hash),
                    Some(&auction)
                ),
            );
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

    let bids: Vec<SimulatedBidEvm> = bids
        .into_iter()
        .filter_map(|b| match b {
            SimulatedBid::Evm(b) => Some(b),
            _ => None,
        })
        .collect();

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

#[tracing::instrument(skip_all)]
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
    provider: Provider<TracedClient>,
    relayer: LocalWallet,
    use_legacy_tx: bool,
    network_id: u64,
) -> SignableExpressRelayContract {
    let transformer = LegacyTxTransformer { use_legacy_tx };
    let client = Arc::new(TransformerMiddleware::new(
        GasOracleMiddleware::new(
            NonceManagerMiddleware::new(
                SignerMiddleware::new(provider.clone(), relayer.clone().with_chain_id(network_id)),
                relayer.address(),
            ),
            EthProviderOracle::new(provider),
        ),
        transformer,
    ));
    SignableExpressRelayContract::new(address, client)
}

async fn submit_auctions(store: Arc<Store>, chain_id: String) {
    let permission_keys = store.get_permission_keys_for_auction(&chain_id).await;

    tracing::info!(
        "Chain: {chain_id} Auctions to process {auction_len}",
        chain_id = chain_id,
        auction_len = permission_keys.len()
    );

    for permission_key in permission_keys.iter() {
        store.task_tracker.spawn({
            let (store, permission_key, chain_id) =
                (store.clone(), permission_key.clone(), chain_id.clone());
            async move {
                if let Err(err) =
                    submit_auction(store, permission_key.clone(), chain_id.clone()).await
                {
                    tracing::error!(
                        "Failed to submit auction: {:?} - permission_key: {:?} - chain_id: {:?}",
                        err,
                        permission_key,
                        chain_id
                    );
                }
            }
        });
    }
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
    tracing::info!(chain_id = chain_id, "Starting transaction submitter...");
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

#[derive(Serialize, Deserialize, ToSchema, Clone, Debug)]
pub struct BidEvm {
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

#[derive(Serialize, Deserialize, ToSchema, Clone, Debug)]
pub struct BidSvm {
    /// The chain id to bid on.
    #[schema(example = "solana", value_type = String)]
    pub chain_id:    ChainId,
    /// The transaction for bid.
    #[schema(example = "SGVsbG8sIFdvcmxkIQ==", value_type = String)]
    #[serde(with = "crate::serde::transaction_svm")]
    pub transaction: VersionedTransaction,
}

#[derive(Serialize, ToSchema, Debug, Clone)]
#[serde(untagged)] // Remove tags to avoid key-value wrapping
pub enum Bid {
    Evm(BidEvm),
    Svm(BidSvm),
}

impl<'de> Deserialize<'de> for Bid {
    fn deserialize<D>(d: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value: serde_json::Value = Deserialize::deserialize(d)?;
        match value.get("transaction") {
            Some(_) => {
                let bid_svm: BidSvm =
                    serde_path_to_error::deserialize(&value).map_err(serde::de::Error::custom)?;
                Ok(Bid::Svm(bid_svm))
            }
            None => {
                let evm_bid: BidEvm =
                    serde_path_to_error::deserialize(&value).map_err(serde::de::Error::custom)?;
                Ok(Bid::Evm(evm_bid))
            }
        }
    }
}

// For now, we are only supporting the EIP1559 enabled networks
async fn verify_bid_exceeds_gas_cost<G>(
    estimated_gas: U256,
    oracle: G,
    bid_amount: U256,
) -> Result<(), RestError>
where
    G: GasOracle,
{
    let (maximum_gas_fee, priority_fee) = oracle
        .estimate_eip1559_fees()
        .await
        .map_err(|_| RestError::TemporarilyUnavailable)?;

    // To submit TOTAL_BIDS_PER_AUCTION together, each bid must cover the gas fee for all of the submitted bids.
    // To make sure we cover the estimation errors, we add the priority_fee to the final potential gas fee.
    // Therefore, the bid amount needs to be TOTAL_BIDS_PER_AUCTION times per potential gas fee.
    let potential_gas_fee = maximum_gas_fee * U256::from(TOTAL_BIDS_PER_AUCTION) + priority_fee;
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
    chain_store: &ChainStoreEvm,
    estimated_gas: U256,
    multiplier: U256,
) -> Result<(), RestError> {
    if chain_store.block_gas_limit < estimated_gas * multiplier {
        let maximum_allowed_gas = chain_store.block_gas_limit / multiplier;
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

// As we submit bids together for an auction, the bid is limited as follows:
// 1. The bid amount should cover gas fees for all bids included in the submission.
// 2. Depending on the maximum number of bids in the auction, the transaction size for the bid is limited.
// 3. Depending on the maximum number of bids in the auction, the gas consumption for the bid is limited.
#[tracing::instrument(skip_all)]
pub async fn handle_bid(
    store: Arc<Store>,
    bid: BidEvm,
    initiation_time: OffsetDateTime,
    auth: Auth,
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
                ContractError::MiddlewareError { e: _ } => Err(RestError::TemporarilyUnavailable),
                ContractError::ProviderError { e: _ } => Err(RestError::TemporarilyUnavailable),
                _ => Err(RestError::BadParameters(format!("Error: {}", e))),
            };
        }
    }

    let estimated_gas = call.estimate_gas().await.map_err(|e| {
        tracing::error!("Error while estimating gas: {:?}", e);
        RestError::TemporarilyUnavailable
    })?;

    verify_bid_exceeds_gas_cost(
        estimated_gas,
        EthProviderOracle::new(chain_store.provider.clone()),
        bid.amount,
    )
    .await?;
    // The transaction body size will be automatically limited when the gas is limited.
    verify_bid_under_gas_limit(
        chain_store,
        estimated_gas,
        U256::from(TOTAL_BIDS_PER_AUCTION * 2),
    )
    .await?;

    let core_fields = SimulatedBidCoreFields::new(
        bid.amount,
        bid.chain_id,
        bid.permission_key,
        initiation_time,
        auth,
    );
    let simulated_bid = SimulatedBidEvm {
        core_fields:     core_fields.clone(),
        target_contract: bid.target_contract,
        target_calldata: bid.target_calldata.clone(),
        // Add a 25% more for estimation errors
        gas_limit:       estimated_gas * U256::from(125) / U256::from(100),
    };
    store.add_bid(simulated_bid.into()).await?;
    Ok(core_fields.id)
}

pub async fn run_tracker_loop(store: Arc<Store>, chain_id: String) -> Result<()> {
    tracing::info!(chain_id = chain_id, "Starting tracker...");
    let chain_store = store
        .chains
        .get(&chain_id)
        .ok_or(anyhow!("Chain not found: {}", chain_id))?;

    let mut exit_check_interval = tokio::time::interval(EXIT_CHECK_INTERVAL);

    // this should be replaced by a subscription to the chain and trigger on new blocks
    let mut submission_interval = tokio::time::interval(Duration::from_secs(10));
    while !SHOULD_EXIT.load(Ordering::Acquire) {
        tokio::select! {
            _ = submission_interval.tick() => {
                match chain_store.provider.get_balance(store.relayer.address(), None).await {
                    Ok(r) => {
                        // This conversion to u128 is fine as the total balance will never cross the limits
                        // of u128 practically.
                        // The f64 conversion is made to be able to serve metrics within the constraints of Prometheus.
                        // The balance is in wei, so we need to divide by 1e18 to convert it to eth.
                        let balance = r.as_u128() as f64 / 1e18;
                        let label = [
                            ("chain_id", chain_id.clone()),
                            ("address", format!("{:?}", store.relayer.address())),
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

// Checks that the transaction includes exactly one submit_bid instruction to the Express Relay on-chain program
pub fn verify_submit_bid_instruction_svm(
    chain_store: &ChainStoreSvm,
    transaction: VersionedTransaction,
) -> Result<CompiledInstruction, RestError> {
    let submit_bid_instructions: Vec<CompiledInstruction> = transaction
        .message
        .instructions()
        .iter()
        .filter(|instruction| {
            let program_id = instruction.program_id(transaction.message.static_account_keys());
            if *program_id != chain_store.express_relay_program_id {
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

fn extract_account_svm(
    accounts: &[Pubkey],
    instruction: CompiledInstruction,
    position: usize,
) -> Result<Pubkey, RestError> {
    let account_position = instruction.accounts.get(position).ok_or_else(|| {
        tracing::error!(
            "Account position not found in instruction: {:?} - {}",
            instruction,
            position,
        );
        RestError::BadParameters("Account not found in submit_bid instruction".to_string())
    })?;

    let account_position: usize = (*account_position).into();
    let account = accounts.get(account_position).ok_or_else(|| {
        tracing::error!(
            "Account not found in transaction accounts: {:?} - {}",
            accounts,
            account_position,
        );
        RestError::BadParameters("Account not found in transaction accounts".to_string())
    })?;

    Ok(*account)
}

fn extract_bid_data_svm(
    express_relay_svm: ExpressRelaySvm,
    accounts: &[Pubkey],
    instruction: CompiledInstruction,
) -> Result<(u64, PermissionKey), RestError> {
    let discriminator = express_relay_svm::instruction::SubmitBid::discriminator();
    let submit_bid_data = express_relay_svm::SubmitBidArgs::try_from_slice(
        &instruction.data.as_slice()[discriminator.len()..],
    )
    .map_err(|e| RestError::BadParameters(format!("Invalid submit_bid instruction data: {}", e)))?;

    let permission_account = extract_account_svm(
        accounts,
        instruction.clone(),
        express_relay_svm.permission_account_position,
    )?;
    let router_account = extract_account_svm(
        accounts,
        instruction.clone(),
        express_relay_svm.router_account_position,
    )?;

    let concat = [permission_account.to_bytes(), router_account.to_bytes()].concat();
    Ok((submit_bid_data.bid_amount, concat.into()))
}

#[tracing::instrument(skip_all)]
pub async fn handle_bid_svm(
    store: Arc<Store>,
    bid: BidSvm,
    initiation_time: OffsetDateTime,
    auth: Auth,
) -> result::Result<Uuid, RestError> {
    let chain_store = store
        .chains_svm
        .get(&bid.chain_id)
        .ok_or(RestError::InvalidChainId)?;

    let submit_bid_instruction =
        verify_submit_bid_instruction_svm(chain_store, bid.transaction.clone())?;
    let (bid_amount, permission_key) = extract_bid_data_svm(
        store.express_relay_svm.clone(),
        bid.transaction.message.static_account_keys(),
        submit_bid_instruction,
    )?;

    verify_signatures_svm(&bid, &store.express_relay_svm.relayer.pubkey())?;
    simulate_bid_svm(chain_store, &bid).await?;

    let core_fields = SimulatedBidCoreFields::new(
        U256::from(bid_amount),
        bid.chain_id,
        permission_key,
        initiation_time,
        auth,
    );
    let simulated_bid = SimulatedBidSvm {
        core_fields: core_fields.clone(),
        transaction: bid.transaction,
    };
    store.add_bid(simulated_bid.clone().into()).await?;
    Ok(core_fields.id)
}

fn verify_signatures_svm(bid: &BidSvm, relayer_pubkey: &Pubkey) -> Result<(), RestError> {
    let message_bytes = bid.transaction.message.serialize();
    let all_signatures_valid = bid
        .transaction
        .signatures
        .iter()
        .zip(bid.transaction.message.static_account_keys().iter())
        .all(|(signature, pubkey)| {
            signature.verify(pubkey.as_ref(), &message_bytes) || pubkey.eq(relayer_pubkey)
        });

    match all_signatures_valid {
        true => Ok(()),
        false => Err(RestError::BadParameters("Invalid signatures".to_string())),
    }
}

async fn simulate_bid_svm(chain_store: &ChainStoreSvm, bid: &BidSvm) -> Result<(), RestError> {
    let response = chain_store
        .client
        .simulate_transaction(&bid.transaction)
        .await;
    let result = response.map_err(|e| {
        tracing::error!("Error while simulating bid: {:?}", e);
        RestError::TemporarilyUnavailable
    })?;
    match result.value.err {
        Some(err) => {
            tracing::error!(
                "Error while simulating bid: {:?}, context: {:?}",
                err,
                result.context
            );
            Err(RestError::SimulationError {
                result: Default::default(),
                reason: err.to_string(),
            })
        }
        None => Ok(()),
    }
}
