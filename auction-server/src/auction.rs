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
            BidStatusEvm,
            BidStatusSvm,
            BidStatusTrait,
            ChainStoreCoreFields,
            ChainStoreEvm,
            ChainStoreSvm,
            ExpressRelaySvm,
            PermissionKey,
            SimulatedBidCoreFields,
            SimulatedBidEvm,
            SimulatedBidSvm,
            SimulatedBidTrait,
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
            SubscriptionStream,
            Ws,
        },
        signers::{
            LocalWallet,
            Signer,
        },
        types::{
            transaction::eip2718::TypedTransaction,
            Address,
            Block,
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
        Stream,
        StreamExt,
    },
    gas_oracle::EthProviderOracle,
    serde::{
        Deserialize,
        Deserializer,
        Serialize,
    },
    solana_client::{
        nonblocking::pubsub_client::PubsubClient,
        rpc_response::SlotInfo,
    },
    solana_sdk::{
        commitment_config::CommitmentConfig,
        instruction::CompiledInstruction,
        pubkey::Pubkey,
        signature::{
            Signature as SignatureSvm,
            Signer as SignerSvm,
        },
        transaction::{
            TransactionError,
            VersionedTransaction,
        },
    },
    sqlx::types::time::OffsetDateTime,
    std::{
        collections::hash_map::Entry,
        fmt::Debug as DebugTrait,
        future::Future,
        ops::Deref,
        pin::Pin,
        result,
        sync::{
            atomic::Ordering,
            Arc,
        },
        time::Duration,
    },
    tokio::sync::{
        Mutex,
        MutexGuard,
    },
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

impl SignableExpressRelayContract {
    pub fn get_relayer_address(&self) -> Address {
        self.client().inner().inner().inner().signer().address()
    }
}

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

fn get_bid_status(
    decoded_log: &MulticallIssuedFilter,
    receipt: &TransactionReceipt,
) -> BidStatusEvm {
    match decoded_log.multicall_status.external_success {
        true => BidStatusEvm::Won {
            index:  decoded_log.multicall_index.as_u32(),
            result: receipt.transaction_hash,
        },
        false => BidStatusEvm::Lost {
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

// An auction is ready if there are any bids with a lifetime of AUCTION_MINIMUM_LIFETIME
fn is_ready_for_auction<T: ChainStore>(
    bids: Vec<T::SimulatedBid>,
    bid_collection_time: OffsetDateTime,
) -> bool {
    bids.into_iter()
        .any(|bid| bid_collection_time - bid.initiation_time > T::AUCTION_MINIMUM_LIFETIME)
}

async fn conclude_submitted_auction<T: ChainStore>(
    store: Arc<Store>,
    chain_store: &T,
    auction: models::Auction,
) -> Result<()> {
    if let Some(tx_hash) = auction.tx_hash.clone() {
        let bids: Vec<T::SimulatedBid> = chain_store
            .bids_for_submitted_auction(auction.clone())
            .await;
        if let Some(bid_statuses) = chain_store.get_bid_results(bids.clone(), tx_hash).await? {
            let auction = store
                .conclude_auction(auction)
                .await
                .map_err(|e| anyhow!("Failed to conclude auction: {:?}", e))?;

            join_all(bid_statuses.iter().enumerate().map(|(index, bid_status)| {
                let (bids, store, auction, bid_statuses) = (bids.clone(), store.clone(), auction.clone(), bid_statuses.clone());
                async move {
                    match bids.get(index) {
                        Some(bid) => {
                            if let Err(err) = store.broadcast_bid_status_and_update(chain_store, bid.clone(), bid_status.clone(), Some(&auction)).await {
                                tracing::error!("Failed to broadcast bid status: {:?} - bid: {:?}", err, bid);
                            }
                        }
                        None => tracing::error!("Bids array is smaller than statuses array. bids: {:?} - statuses: {:?} - auction: {:?} ", bids, bid_statuses, auction),
                    }
                }
            }))
            .await;
            chain_store.remove_submitted_auction(auction).await;
        }
    }
    Ok(())
}

async fn conclude_submitted_auctions<T: ChainStore>(
    store: Arc<Store>,
    chain_store: &T,
    chain_id: String,
) {
    let auctions = chain_store.get_submitted_auctions().await;

    tracing::info!(
        "Chain: {chain_id} Auctions to conclude {auction_len}",
        chain_id = chain_id,
        auction_len = auctions.len()
    );

    for auction in auctions.iter() {
        store.task_tracker.spawn({
            let (store, auction) = (store.clone(), auction.clone());
            async move {
                let result = match auction.chain_type {
                    models::ChainType::Evm => match store.chains.get(&auction.chain_id) {
                        Some(chain_store) => {
                            conclude_submitted_auction(store.clone(), chain_store, auction.clone())
                                .await
                        }
                        None => Err(anyhow!("Chain not found: {}", auction.chain_id)),
                    },
                    models::ChainType::Svm => match store.chains_svm.get(&auction.chain_id) {
                        Some(chain_store) => {
                            conclude_submitted_auction(store.clone(), chain_store, auction.clone())
                                .await
                        }
                        None => Err(anyhow!("Chain not found: {}", auction.chain_id)),
                    },
                };

                if let Err(err) = result {
                    tracing::error!(
                        "Failed to conclude auction: {:?} - auction: {:?}",
                        err,
                        auction
                    );
                }
            }
        });
    }
}

async fn broadcast_submitted_bids<T: ChainStore>(
    store: Arc<Store>,
    chain_store: &T,
    bids: Vec<T::SimulatedBid>,
    tx_hash: <<T::SimulatedBid as SimulatedBidTrait>::StatusType as BidStatusTrait>::TxHash,
    auction: models::Auction,
) {
    join_all(bids.iter().enumerate().map(|(i, bid)| {
        let (store, auction, index, tx_hash) =
            (store.clone(), auction.clone(), i as u32, tx_hash.clone());
        async move {
            match <T::SimulatedBid as SimulatedBidTrait>::get_bid_status(
                models::BidStatus::Submitted,
                Some(index),
                Some(tx_hash),
            ) {
                Ok(status) => {
                    if let Err(err) = store
                        .broadcast_bid_status_and_update(
                            chain_store,
                            bid.clone(),
                            status,
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
                Err(err) => {
                    tracing::error!("Failed to get bid status: {:?} - bid: {:?}", err, bid);
                }
            }
        }
    }))
    .await;
}

async fn broadcast_lost_bids<T: ChainStore>(
    store: Arc<Store>,
    chain_store: &T,
    bids: Vec<T::SimulatedBid>,
    submitted_bids: Vec<T::SimulatedBid>,
    tx_hash: Option<<<T::SimulatedBid as SimulatedBidTrait>::StatusType as BidStatusTrait>::TxHash>,
    auction: Option<&models::Auction>,
) {
    join_all(bids.iter().filter_map(|bid| {
        if submitted_bids
            .iter()
            .any(|submitted_bid| bid.id == submitted_bid.id)
        {
            return None;
        }

        let (store, tx_hash) = (store.clone(), tx_hash.clone());
        Some(async move {
            match <T::SimulatedBid as SimulatedBidTrait>::get_bid_status(
                models::BidStatus::Lost,
                None,
                tx_hash,
            ) {
                Ok(status) => {
                    if let Err(err) = store
                        .broadcast_bid_status_and_update(chain_store, bid.clone(), status, auction)
                        .await
                    {
                        tracing::error!(
                            "Failed to broadcast bid status: {:?} - bid: {:?}",
                            err,
                            bid
                        );
                    }
                }
                Err(err) => {
                    tracing::error!("Failed to get bid status: {:?} - bid: {:?}", err, bid);
                }
            }
        })
    }))
    .await;
}

async fn submit_auction_for_bids<'a, T: ChainStore>(
    bids: Vec<T::SimulatedBid>,
    bid_collection_time: OffsetDateTime,
    permission_key: Bytes,
    chain_id: String,
    store: Arc<Store>,
    chain_store: &T,
    _auction_mutex_gaurd: MutexGuard<'a, ()>,
) -> Result<()> {
    let bids: Vec<T::SimulatedBid> = bids
        .into_iter()
        .filter(|bid| models::BidStatus::Pending == bid.status.clone().into())
        .collect();

    if bids.is_empty() {
        return Ok(());
    }

    if !is_ready_for_auction::<T>(bids.clone(), bid_collection_time) {
        tracing::info!("Auction for {} is not ready yet", permission_key);
        return Ok(());
    }

    let winner_bids = chain_store
        .get_winner_bids(&bids, permission_key.clone())
        .await?;
    if winner_bids.is_empty() {
        broadcast_lost_bids(store.clone(), chain_store, bids, winner_bids, None, None).await;
        return Ok(());
    }

    let mut auction = store
        .init_auction::<T>(
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

    match chain_store
        .submit_bids(permission_key.clone(), winner_bids.clone())
        .await
    {
        Ok(tx_hash) => {
            tracing::debug!("Submitted transaction: {:?}", tx_hash);
            let converted_tx_hash = <<T::SimulatedBid as SimulatedBidTrait>::StatusType as BidStatusTrait>::convert_tx_hash(&tx_hash);
            auction = store
                .submit_auction(chain_store, auction, converted_tx_hash)
                .await?;
            tokio::join!(
                broadcast_submitted_bids(
                    store.clone(),
                    chain_store,
                    winner_bids.clone(),
                    tx_hash.clone(),
                    auction.clone()
                ),
                broadcast_lost_bids(
                    store.clone(),
                    chain_store,
                    bids,
                    winner_bids,
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

async fn submit_auction_for_lock<T: ChainStore>(
    store: Arc<Store>,
    chain_store: &T,
    permission_key: Bytes,
    chain_id: String,
    auction_lock: AuctionLock,
) -> Result<()> {
    let acquired_lock = auction_lock.lock().await;

    let bid_collection_time: OffsetDateTime = OffsetDateTime::now_utc();
    let bids: Vec<T::SimulatedBid> = chain_store.get_bids(&permission_key).await;

    submit_auction_for_bids(
        bids.clone(),
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
async fn submit_auction<T: ChainStore>(
    store: Arc<Store>,
    chain_store: &T,
    permission_key: Bytes,
    chain_id: String,
) -> Result<()> {
    let auction_lock = chain_store.get_auction_lock(permission_key.clone()).await;
    let result = submit_auction_for_lock(
        store.clone(),
        chain_store,
        permission_key.clone(),
        chain_id,
        auction_lock,
    )
    .await;
    chain_store.remove_auction_lock(&permission_key).await;
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

async fn submit_auctions<T: ChainStore>(store: Arc<Store>, chain_store: &T, chain_id: String) {
    let permission_keys = chain_store.get_permission_keys_for_auction().await;

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
                let result = match T::CHAIN_TYPE {
                    models::ChainType::Evm => match store.chains.get(&chain_id) {
                        Some(chain_store) => {
                            submit_auction(
                                store.clone(),
                                chain_store,
                                permission_key.clone(),
                                chain_id.clone(),
                            )
                            .await
                        }
                        None => Err(anyhow!("Chain not found: {}", chain_id)),
                    },
                    models::ChainType::Svm => match store.chains_svm.get(&chain_id) {
                        Some(chain_store) => {
                            submit_auction(
                                store.clone(),
                                chain_store,
                                permission_key.clone(),
                                chain_id.clone(),
                            )
                            .await
                        }
                        None => Err(anyhow!("Chain not found: {}", chain_id)),
                    },
                };
                if let Err(err) = result {
                    tracing::error!(
                        "Failed to submit auction: {:?} - permission_key: {:?} - chain_id: {:?}",
                        err,
                        permission_key,
                        chain_id,
                    );
                }
            }
        });
    }
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
        chain_store.express_relay_contract.get_relayer_address(),
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
        U256::from(TOTAL_BIDS_PER_AUCTION),
    )
    .await?;

    let core_fields = SimulatedBidCoreFields::new(
        bid.amount,
        bid.chain_id,
        BidStatusEvm::Pending,
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
    store.add_bid(chain_store, simulated_bid).await?;
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
                match chain_store.provider.get_balance(chain_store.express_relay_contract.get_relayer_address(), None).await {
                    Ok(r) => {
                        // This conversion to u128 is fine as the total balance will never cross the limits
                        // of u128 practically.
                        // The f64 conversion is made to be able to serve metrics within the constraints of Prometheus.
                        // The balance is in wei, so we need to divide by 1e18 to convert it to eth.
                        let balance = r.as_u128() as f64 / 1e18;
                        let label = [
                            ("chain_id", chain_id.clone()),
                            ("address", format!("{:?}", chain_store.express_relay_contract.get_relayer_address())),
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
            if *program_id != chain_store.config.express_relay_program_id {
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
        chain_store.express_relay_svm.clone(),
        bid.transaction.message.static_account_keys(),
        submit_bid_instruction,
    )?;

    verify_signatures_svm(&bid, &chain_store.express_relay_svm.relayer.pubkey())?;
    simulate_bid_svm(chain_store, &bid).await?;

    let core_fields = SimulatedBidCoreFields::new(
        U256::from(bid_amount),
        bid.chain_id,
        BidStatusSvm::Pending,
        permission_key,
        initiation_time,
        auth,
    );
    let simulated_bid = SimulatedBidSvm {
        core_fields: core_fields.clone(),
        transaction: bid.transaction,
    };
    store.add_bid(chain_store, simulated_bid).await?;
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

/// The trait for the chain store to be implemented for each chain type
/// These functions are chain specific and should be implemented for each chain in order to handle auctions
pub trait ChainStore: Deref<Target = ChainStoreCoreFields<Self::SimulatedBid>> {
    /// The trigger type for the chain. This is the type that is used to trigger the auction submission and conclusion
    type Trigger: DebugTrait;
    /// The trigger stream type when subscribing to new triggers on the ws client for the chain
    type TriggerStream<'a>: Stream<Item = Self::Trigger> + Unpin + Send + 'a;
    /// The ws client type for the chain
    type WsClient;
    /// The simulated bid type for the chain
    type SimulatedBid: SimulatedBidTrait;
    /// The conclusion result type when try to conclude the auction for the chain
    type ConclusionResult;

    /// The chain type for the chain
    const CHAIN_TYPE: models::ChainType;
    /// The minimum lifetime for an auction. If any bid for auction is older than this, the auction is ready to be submitted.
    const AUCTION_MINIMUM_LIFETIME: Duration;

    /// Get the ws client for the chain
    fn get_ws_client(&self) -> impl Future<Output = Result<Self::WsClient>>;
    /// Get the trigger stream for the ws client to subscribe to new triggers
    fn get_trigger_stream<'a>(
        client: &'a Self::WsClient,
    ) -> impl Future<Output = Result<Self::TriggerStream<'a>>>;
    /// Get the winner bids for the auction. Sorting bids by bid amount and simulating the bids to determine the winner bids.
    fn get_winner_bids(
        &self,
        bids: &[Self::SimulatedBid],
        permission_key: Bytes,
    ) -> impl Future<Output = Result<Vec<Self::SimulatedBid>>>;
    /// Submit the bids for the auction on the chain
    fn submit_bids(
        &self,
        permission_key: Bytes,
        bids: Vec<Self::SimulatedBid>,
    ) -> impl Future<
        Output = Result<
            <<Self::SimulatedBid as SimulatedBidTrait>::StatusType as BidStatusTrait>::TxHash,
        >,
    >;
    /// Get the bid results for the bids submitted for the auction after the transaction is concluded. Order of the returned BidStatus is as same as the order of the bids
    fn get_bid_results(
        &self,
        bids: Vec<Self::SimulatedBid>,
        tx_hash: Vec<u8>,
    ) -> impl Future<
        Output = Result<Option<Vec<<Self::SimulatedBid as SimulatedBidTrait>::StatusType>>>,
    > + Send;

    async fn get_bids(&self, key: &PermissionKey) -> Vec<Self::SimulatedBid> {
        self.bids.read().await.get(key).cloned().unwrap_or_default()
    }

    async fn add_bid(&self, bid: Self::SimulatedBid) {
        self.bids
            .write()
            .await
            .entry(bid.permission_key.clone())
            .or_insert_with(Vec::new)
            .push(bid);
    }

    async fn remove_bid(&self, bid: Self::SimulatedBid) {
        let mut write_guard = self.bids.write().await;
        let key = bid.permission_key.clone();
        if let Entry::Occupied(mut entry) = write_guard.entry(key.clone()) {
            let bids = entry.get_mut();
            bids.retain(|b| b.id != bid.id);
            if bids.is_empty() {
                entry.remove();
            }
        }
    }

    async fn update_bid(&self, bid: Self::SimulatedBid) {
        let mut write_guard = self.bids.write().await;
        let key = bid.permission_key.clone();
        match write_guard.entry(key.clone()) {
            Entry::Occupied(mut entry) => {
                let bids = entry.get_mut();
                match bids.iter().position(|b| b.id == bid.id) {
                    Some(index) => bids[index] = bid,
                    None => {
                        tracing::error!("Update bid failed - bid not found for: {:?}", bid);
                    }
                }
            }
            Entry::Vacant(_) => {
                tracing::error!("Update bid failed - entry not found for key: {:?}", key);
            }
        }
    }

    async fn add_submitted_auction(&self, auction: models::Auction) {
        self.submitted_auctions.write().await.push(auction.clone());
    }

    async fn get_submitted_auctions(&self) -> Vec<models::Auction> {
        self.submitted_auctions.read().await.to_vec()
    }

    async fn get_permission_keys_for_auction(&self) -> Vec<PermissionKey> {
        self.bids.read().await.keys().cloned().collect()
    }

    async fn bids_for_submitted_auction(
        &self,
        auction: models::Auction,
    ) -> Vec<Self::SimulatedBid> {
        let bids = self.get_bids(&auction.permission_key.clone().into()).await;
        match auction.tx_hash {
            Some(tx_hash) => bids
                .into_iter()
                .filter(|bid| {
                    if models::BidStatus::Submitted == bid.status.clone().into() {
                        if let Some(status_tx_hash) = bid.status.get_tx_hash() {
                            return <Self::SimulatedBid as SimulatedBidTrait>::StatusType::convert_tx_hash(&status_tx_hash)
                                == tx_hash;
                        }
                    }
                    false
                })
                .collect(),
            None => vec![],
        }
    }

    async fn remove_submitted_auction(&self, auction: models::Auction) {
        if !self
            .bids_for_submitted_auction(auction.clone())
            .await
            .is_empty()
        {
            return;
        }

        let mut write_guard = self.submitted_auctions.write().await;
        write_guard.retain(|a| a.id != auction.id);
    }

    async fn get_auction_lock(&self, key: PermissionKey) -> AuctionLock {
        self.auction_lock
            .lock()
            .await
            .entry(key)
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    async fn remove_auction_lock(&self, key: &PermissionKey) {
        let mut mutex_gaurd = self.auction_lock.lock().await;
        let auction_lock = mutex_gaurd.get(key);
        if let Some(auction_lock) = auction_lock {
            // Whenever there is no thread borrowing a lock for this key, we can remove it from the locks HashMap.
            if Arc::strong_count(auction_lock) == 1 {
                mutex_gaurd.remove(key);
            }
        }
    }
}

// While we are submitting bids together, increasing this number will have the following effects:
// 1. There will be more gas required for the transaction, which will result in a higher minimum bid amount.
// 2. The transaction size limit will be reduced for each bid.
// 3. Gas consumption limit will decrease for the bid
const TOTAL_BIDS_PER_AUCTION: usize = 3;

impl ChainStore for ChainStoreEvm {
    type Trigger = Block<H256>;
    type TriggerStream<'a> = SubscriptionStream<'a, Ws, Block<H256>>;
    type WsClient = Provider<Ws>;
    type SimulatedBid = SimulatedBidEvm;
    type ConclusionResult = TransactionReceipt;

    const CHAIN_TYPE: models::ChainType = models::ChainType::Evm;
    const AUCTION_MINIMUM_LIFETIME: Duration = Duration::from_secs(1);

    async fn get_ws_client(&self) -> Result<Self::WsClient> {
        let ws = Ws::connect(self.config.geth_ws_addr.clone()).await?;
        Ok(Provider::new(ws))
    }

    async fn get_trigger_stream<'a>(client: &'a Self::WsClient) -> Result<Self::TriggerStream<'a>> {
        let block_stream = client.subscribe_blocks().await?;
        Ok(block_stream)
    }

    #[tracing::instrument(skip_all)]
    async fn get_winner_bids(
        &self,
        bids: &[Self::SimulatedBid],
        permission_key: Bytes,
    ) -> Result<Vec<Self::SimulatedBid>> {
        // TODO How we want to perform simulation, pruning, and determination
        if bids.is_empty() {
            return Ok(vec![]);
        }

        let mut bids = bids.to_owned();
        bids.sort_by(|a, b| b.core_fields.bid_amount.cmp(&a.core_fields.bid_amount));
        let bids: Vec<SimulatedBidEvm> = bids.into_iter().take(TOTAL_BIDS_PER_AUCTION).collect();
        let simulation_result = get_simulation_call(
            self.express_relay_contract.get_relayer_address(),
            self.provider.clone(),
            self.config.clone(),
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
            Some(index) => Ok(bids.into_iter().skip(index).collect()),
            None => Ok(vec![]),
        }
    }

    #[tracing::instrument(skip_all)]
    async fn submit_bids(
        &self,
        permission_key: Bytes,
        bids: Vec<Self::SimulatedBid>,
    ) -> Result<<<Self::SimulatedBid as SimulatedBidTrait>::StatusType as BidStatusTrait>::TxHash>
    {
        let gas_estimate = bids.iter().fold(U256::zero(), |sum, b| sum + b.gas_limit);
        let tx_hash = self
            .express_relay_contract
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
        bids: Vec<Self::SimulatedBid>,
        tx_hash: Vec<u8>,
    ) -> Result<Option<Vec<<Self::SimulatedBid as SimulatedBidTrait>::StatusType>>> {
        let tx_hash = H256::from_slice(tx_hash.as_slice());
        let reciept = self
            .provider
            .get_transaction_receipt(tx_hash)
            .await
            .map_err(|e| anyhow!("Failed to get transaction receipt: {:?}", e))?;
        match reciept {
            Some(receipt) => {
                let decoded_logs = decode_logs_for_receipt(&receipt);
                Ok(Some(
                    bids.iter()
                        .map(|b| {
                            match decoded_logs.iter().find(|decoded_log| {
                                Uuid::from_bytes(decoded_log.bid_id) == b.core_fields.id
                            }) {
                                Some(decoded_log) => get_bid_status(decoded_log, &receipt),
                                None => BidStatusEvm::Lost {
                                    result: Some(tx_hash),
                                    index:  None,
                                },
                            }
                        })
                        .collect(),
                ))
            }
            None => Ok(None),
        }
    }
}

impl ChainStore for ChainStoreSvm {
    type Trigger = SlotInfo;
    type TriggerStream<'a> = Pin<Box<dyn Stream<Item = Self::Trigger> + Send + 'a>>;
    type WsClient = PubsubClient;
    type SimulatedBid = SimulatedBidSvm;
    type ConclusionResult = result::Result<(), TransactionError>;

    const CHAIN_TYPE: models::ChainType = models::ChainType::Svm;
    const AUCTION_MINIMUM_LIFETIME: Duration = Duration::from_millis(400);

    async fn get_ws_client(&self) -> Result<Self::WsClient> {
        PubsubClient::new(&self.config.ws_addr).await.map_err(|e| {
            tracing::error!("Error while creating svm pub sub client: {:?}", e);
            anyhow!(e)
        })
    }

    async fn get_trigger_stream<'a>(client: &'a Self::WsClient) -> Result<Self::TriggerStream<'a>> {
        let (slot_subscribe, _) = client.slot_subscribe().await?;
        Ok(slot_subscribe)
    }

    async fn get_winner_bids(
        &self,
        bids: &[Self::SimulatedBid],
        _permission_key: Bytes,
    ) -> Result<Vec<Self::SimulatedBid>> {
        let mut bids = bids.to_owned();
        bids.sort_by(|a, b| b.core_fields.bid_amount.cmp(&a.core_fields.bid_amount));
        for bid in bids.iter() {
            match simulate_bid_svm(
                self,
                &BidSvm {
                    chain_id:    bid.core_fields.chain_id.clone(),
                    transaction: bid.transaction.clone(),
                },
            )
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
        _permission_key: Bytes,
        bids: Vec<Self::SimulatedBid>,
    ) -> Result<<<Self::SimulatedBid as SimulatedBidTrait>::StatusType as BidStatusTrait>::TxHash>
    {
        let relayer = self.express_relay_svm.relayer.clone();
        let mut bid = bids[0].clone();
        let serialized_message = bid.transaction.message.serialize();
        let relayer_signature_pos = bid
            .transaction
            .message
            .static_account_keys()
            .iter()
            .position(|p| p.eq(&relayer.pubkey()))
            .expect("Relayer not found in static account keys");
        bid.transaction.signatures[relayer_signature_pos] =
            relayer.sign_message(&serialized_message);
        match self.client.send_transaction(&bid.transaction).await {
            Ok(response) => Ok(response),
            Err(e) => {
                tracing::error!("Error while submitting bid: {:?}", e);
                Err(anyhow!(e))
            }
        }
    }

    async fn get_bid_results(
        &self,
        bids: Vec<Self::SimulatedBid>,
        tx_hash: Vec<u8>,
    ) -> Result<Option<Vec<<Self::SimulatedBid as SimulatedBidTrait>::StatusType>>> {
        let tx_hash: SignatureSvm = tx_hash
            .try_into()
            .map_err(|_| anyhow!("Invalid svm signature"))?;
        if bids.len() != 1 {
            return Err(anyhow!("Invalid number of bids: {}", bids.len()));
        }

        let status = self
            .client
            .get_signature_status_with_commitment(&tx_hash, CommitmentConfig::confirmed())
            .await?;

        match status {
            Some(res) => Ok(Some(vec![match res {
                Ok(()) => BidStatusSvm::Won { result: tx_hash },
                Err(_) => BidStatusSvm::Lost {
                    result: Some(tx_hash),
                },
            }])),
            None => {
                // not yet confirmed
                Ok(None)
            }
        }
    }
}

impl Deref for ChainStoreEvm {
    type Target = ChainStoreCoreFields<SimulatedBidEvm>;

    fn deref(&self) -> &Self::Target {
        &self.core_fields
    }
}

impl Deref for ChainStoreSvm {
    type Target = ChainStoreCoreFields<SimulatedBidSvm>;

    fn deref(&self) -> &Self::Target {
        &self.core_fields
    }
}

async fn run_submission_loop<T: ChainStore>(
    store: Arc<Store>,
    chain_store: &T,
    chain_id: String,
) -> Result<()> {
    tracing::info!(chain_id = chain_id, "Starting transaction submitter...");
    let mut exit_check_interval = tokio::time::interval(EXIT_CHECK_INTERVAL);

    let ws_client = chain_store.get_ws_client().await?;
    let mut stream = T::get_trigger_stream(&ws_client).await?;

    while !SHOULD_EXIT.load(Ordering::Acquire) {
        tokio::select! {
            trigger = stream.next() => {
                if trigger.is_none() {
                    return Err(anyhow!("Trigger stream ended for chain: {}", chain_id));
                }

                tracing::debug!("New trigger received for {} at {}: {:?}", chain_id.clone(), OffsetDateTime::now_utc(), trigger);
                store.task_tracker.spawn({
                    let (store, chain_id) = (store.clone(), chain_id.clone());
                    async move {
                        match T::CHAIN_TYPE {
                            models::ChainType::Evm => {
                                if let Some(chain_store) = store
                                    .chains
                                    .get(&chain_id) {
                                        submit_auctions(store.clone(), chain_store, chain_id).await
                                    }
                            }
                            models::ChainType::Svm => {
                                if let Some(chain_store) = store
                                    .chains
                                    .get(&chain_id) {
                                        submit_auctions(store.clone(), chain_store, chain_id).await
                                    }
                            }
                        }
                    }
                });

                store.task_tracker.spawn({
                    let (store, chain_id) = (store.clone(), chain_id.clone());
                    async move {
                        match T::CHAIN_TYPE {
                            models::ChainType::Evm => {
                                if let Some(chain_store) = store
                                    .chains
                                    .get(&chain_id) {
                                        conclude_submitted_auctions(store.clone(), chain_store, chain_id.clone()).await
                                    }
                            }
                            models::ChainType::Svm => {
                                if let Some(chain_store) = store
                                    .chains
                                    .get(&chain_id) {
                                        conclude_submitted_auctions(store.clone(), chain_store, chain_id.clone()).await
                                    }
                            }
                        }
                    }
                });
            }
            _ = exit_check_interval.tick() => {}
        }
    }
    tracing::info!("Shutting down transaction submitter...");
    Ok(())
}

pub async fn run_submission_loop_evm(store: Arc<Store>, chain_id: String) -> Result<()> {
    let chain_store = store
        .chains
        .get(&chain_id)
        .ok_or(anyhow!("Chain not found: {}", chain_id))?;
    run_submission_loop(store.clone(), chain_store, chain_id).await
}

pub async fn run_submission_loop_svm(store: Arc<Store>, chain_id: String) -> Result<()> {
    let chain_store = store
        .chains_svm
        .get(&chain_id)
        .ok_or(anyhow!("Chain not found: {}", chain_id))?;
    run_submission_loop(store.clone(), chain_store, chain_id).await
}
