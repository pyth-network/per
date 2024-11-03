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
        kernel::entities::{
            PermissionKey,
            PermissionKeySvm,
        },
        models,
        opportunity::service::{
            get_live_opportunities::GetOpportunitiesInput,
            ChainType as OpportunityChainType,
            ChainTypeEvm as OpportunityChainTypeEvm,
            ChainTypeSvm as OpportunityChainTypeSvm,
            Service as OpportunityService,
        },
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
            LookupTableCache,
            SimulatedBidCoreFields,
            SimulatedBidEvm,
            SimulatedBidSvm,
            SimulatedBidTrait,
            Store,
            StoreNew,
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
    axum::async_trait,
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
        nonblocking::{
            pubsub_client::PubsubClient,
            rpc_client::RpcClient,
        },
        rpc_config::{
            RpcTransactionLogsConfig,
            RpcTransactionLogsFilter,
        },
        rpc_response::SlotInfo,
    },
    solana_sdk::{
        address_lookup_table::state::AddressLookupTable,
        commitment_config::CommitmentConfig,
        instruction::CompiledInstruction,
        pubkey::Pubkey,
        signature::{
            Keypair,
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
            bid_amount: bid.bid_amount,
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

async fn conclude_submitted_auctions<T: ChainStore + 'static>(
    store: Arc<Store>,
    chain_store: Arc<T>,
) {
    let auctions = chain_store.get_submitted_auctions().await;

    // tracing::info!(
    //     "Chain: {chain_id} Auctions to conclude {auction_len}",
    //     chain_id = chain_store.get_name(),
    //     auction_len = auctions.len()
    // );

    for auction in auctions.iter() {
        store.task_tracker.spawn({
            let (store, auction, chain_store) =
                (store.clone(), auction.clone(), chain_store.clone());
            async move {
                let result = conclude_submitted_auction(
                    store.clone(),
                    chain_store.as_ref(),
                    auction.clone(),
                )
                .await;
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

pub async fn broadcast_submitted_bids<T: ChainStore>(
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

pub async fn broadcast_lost_bids<T: ChainStore>(
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
    store: Arc<Store>,
    chain_store: &T,
    _auction_mutex_gaurd: MutexGuard<'a, ()>,
) -> Result<()> {
    let bids: Vec<T::SimulatedBid> = bids
        .into_iter()
        .filter(|bid| bid.get_status().clone() == models::BidStatus::Pending)
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
            chain_store.get_name().clone(),
            bid_collection_time,
        )
        .await?;

    tracing::info!(
        "Submission for {} on chain {} started at {}",
        permission_key,
        chain_store.get_name(),
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
    auction_lock: AuctionLock,
) -> Result<()> {
    let acquired_lock = auction_lock.lock().await;

    let bid_collection_time: OffsetDateTime = OffsetDateTime::now_utc();
    let bids: Vec<T::SimulatedBid> = chain_store.get_bids(&permission_key).await;

    submit_auction_for_bids(
        bids.clone(),
        bid_collection_time,
        permission_key.clone(),
        store.clone(),
        chain_store,
        acquired_lock,
    )
    .await
}

#[tracing::instrument(skip_all)]
async fn handle_auction<T: ChainStore>(
    store_new: Arc<StoreNew>,
    chain_store: Arc<T>,
    permission_key: Bytes,
) -> Result<()> {
    let store = store_new.store.clone();
    match chain_store
        .get_submission_state(store_new, &permission_key)
        .await
    {
        SubmitType::SubmitByOther => Ok(()),
        SubmitType::SubmitByServer => {
            let auction_lock = chain_store.get_auction_lock(permission_key.clone()).await;
            let result = submit_auction_for_lock(
                store.clone(),
                chain_store.as_ref(),
                permission_key.clone(),
                auction_lock,
            )
            .await;
            chain_store.remove_auction_lock(&permission_key).await;
            result
        }
        SubmitType::Invalid => {
            // Fetch all pending bids and mark them as lost
            let bids = chain_store
                .get_bids(&permission_key)
                .await
                .into_iter()
                .filter(|bid| bid.get_status().clone() == models::BidStatus::Pending)
                .collect();
            broadcast_lost_bids(
                store.clone(),
                chain_store.as_ref(),
                bids,
                vec![],
                None,
                None,
            )
            .await;
            Ok(())
        }
    }
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

async fn handle_auctions<T: ChainStore + 'static>(store_new: Arc<StoreNew>, chain_store: Arc<T>) {
    let permission_keys = chain_store.get_permission_keys_for_auction().await;

    // tracing::info!(
    //     "Chain: {chain_id} Auctions to process {auction_len}",
    //     chain_id = chain_store.get_name(),
    //     auction_len = permission_keys.len()
    // );

    for permission_key in permission_keys.iter() {
        store_new.store.task_tracker.spawn({
            let (store_new, permission_key) = (store_new.clone(), permission_key.clone());
            let chain_store = chain_store.clone();
            async move {
                let result = handle_auction(
                    store_new.clone(),
                    chain_store.clone(),
                    permission_key.clone(),
                )
                .await;
                if let Err(err) = result {
                    tracing::error!(
                        "Failed to submit auction: {:?} - permission_key: {:?} - chain_id: {:?}",
                        err,
                        permission_key,
                        chain_store.get_name(),
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
        .chains_evm
        .get(&bid.chain_id)
        .ok_or(RestError::InvalidChainId)?
        .as_ref();
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

    let core_fields = SimulatedBidCoreFields::new(bid.chain_id, initiation_time, auth);
    let simulated_bid = SimulatedBidEvm {
        core_fields:     core_fields.clone(),
        target_contract: bid.target_contract,
        target_calldata: bid.target_calldata.clone(),
        // Add a 25% more for estimation errors
        gas_limit:       estimated_gas * U256::from(125) / U256::from(100),
        bid_amount:      bid.amount,
        permission_key:  bid.permission_key,
        status:          BidStatusEvm::Pending,
    };
    store.add_bid(chain_store, simulated_bid).await?;
    Ok(core_fields.id)
}

pub async fn run_tracker_loop(chain_store: Arc<ChainStoreEvm>) -> Result<()> {
    tracing::info!(chain_id = chain_store.get_name(), "Starting tracker...");

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
                            ("chain_id", chain_store.get_name().clone()),
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
    express_relay_pid: &Pubkey,
    transaction: VersionedTransaction,
) -> Result<CompiledInstruction, RestError> {
    let submit_bid_instructions: Vec<CompiledInstruction> = transaction
        .message
        .instructions()
        .iter()
        .filter(|instruction| {
            let program_id = instruction.program_id(transaction.message.static_account_keys());
            if program_id != express_relay_pid {
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

async fn extract_account_svm(
    tx: &VersionedTransaction,
    submit_bid_instruction: &CompiledInstruction,
    position: usize,
    lookup_table_cache: &LookupTableCache,
    client: &RpcClient,
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
            find_and_query_lookup_table(
                lookup_accounts,
                account_position_lookups,
                client,
                lookup_table_cache,
            )
            .await
        }
        None => Err(RestError::BadParameters(
            "No lookup tables found in submit_bid instruction".to_string(),
        )),
    }
}

async fn find_and_query_lookup_table(
    lookup_accounts: Vec<(Pubkey, u8)>,
    account_position: usize,
    client: &RpcClient,
    lookup_table_cache: &LookupTableCache,
) -> Result<Pubkey, RestError> {
    let (table_to_query, index_to_query) =
        lookup_accounts.get(account_position).ok_or_else(|| {
            RestError::BadParameters("Lookup table not found in lookup accounts".to_string())
        })?;

    query_lookup_table(
        table_to_query,
        *index_to_query as usize,
        client,
        lookup_table_cache,
    )
    .await
}

async fn query_lookup_table(
    table: &Pubkey,
    index: usize,
    client: &RpcClient,
    lookup_table_cache: &LookupTableCache,
) -> Result<Pubkey, RestError> {
    if let Some(Some(cached_table)) = lookup_table_cache
        .read()
        .await
        .get(table)
        .map(|keys| keys.get(index).cloned())
    {
        return Ok(cached_table);
    }

    let table_data = client
        .get_account_with_commitment(table, CommitmentConfig::processed())
        .await
        .map_err(|_e| RestError::TemporarilyUnavailable)?
        .value
        .ok_or_else(|| RestError::BadParameters("Account not found".to_string()))?;
    let table_data_deserialized =
        AddressLookupTable::deserialize(&table_data.data).map_err(|_e| {
            RestError::BadParameters("Failed deserializing lookup table account data".to_string())
        })?;
    let account = table_data_deserialized
        .addresses
        .get(index)
        .ok_or_else(|| RestError::BadParameters("Account not found in lookup table".to_string()))?;

    let keys_to_cache = table_data_deserialized.addresses.to_vec();
    lookup_table_cache
        .write()
        .await
        .insert(*table, keys_to_cache);

    Ok(*account)
}

pub fn extract_submit_bid_data(
    instruction: &CompiledInstruction,
) -> Result<express_relay_svm::SubmitBidArgs, RestError> {
    let discriminator = express_relay_svm::instruction::SubmitBid::discriminator();
    express_relay_svm::SubmitBidArgs::try_from_slice(
        &instruction.data.as_slice()[discriminator.len()..],
    )
    .map_err(|e| RestError::BadParameters(format!("Invalid submit_bid instruction data: {}", e)))
}

pub struct BidDataSvm {
    pub amount:         u64,
    pub permission_key: PermissionKeySvm,
    pub deadline:       i64,
}

async fn extract_bid_data_svm(
    chain_store: &ChainStoreSvm,
    tx: VersionedTransaction,
    client: &RpcClient,
) -> Result<BidDataSvm, RestError> {
    let submit_bid_instruction = verify_submit_bid_instruction_svm(
        &chain_store.config.express_relay_program_id,
        tx.clone(),
    )?;
    let submit_bid_data = extract_submit_bid_data(&submit_bid_instruction)?;

    let permission_account = extract_account_svm(
        &tx,
        &submit_bid_instruction,
        chain_store.express_relay_svm.permission_account_position,
        &chain_store.lookup_table_cache,
        client,
    )
    .await?;
    let router_account = extract_account_svm(
        &tx,
        &submit_bid_instruction,
        chain_store.express_relay_svm.router_account_position,
        &chain_store.lookup_table_cache,
        client,
    )
    .await?;
    let mut permission_key = [0; 64];
    permission_key[..32].copy_from_slice(&router_account.to_bytes());
    permission_key[32..].copy_from_slice(&permission_account.to_bytes());
    Ok(BidDataSvm {
        amount:         submit_bid_data.bid_amount,
        permission_key: PermissionKeySvm(permission_key),
        deadline:       submit_bid_data.deadline,
    })
}

impl PartialEq<SimulatedBidSvm> for BidSvm {
    fn eq(&self, other: &SimulatedBidSvm) -> bool {
        self.transaction == other.transaction && self.chain_id == other.core_fields.chain_id
    }
}

async fn check_deadline(
    store_new: Arc<StoreNew>,
    chain_store: &ChainStoreSvm,
    permission_key: &Bytes,
    deadline: i64,
) -> Result<(), RestError> {
    let minimum_bid_life_time = match chain_store
        .get_submission_state(store_new, permission_key)
        .await
    {
        SubmitType::SubmitByServer => Some(BID_MINIMUM_LIFE_TIME_SVM_SERVER),
        SubmitType::SubmitByOther => Some(BID_MINIMUM_LIFE_TIME_SVM_OTHER),
        SubmitType::Invalid => None,
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

const BID_MINIMUM_LIFE_TIME_SVM_SERVER: i64 = 5;
const BID_MINIMUM_LIFE_TIME_SVM_OTHER: i64 = 10;

#[tracing::instrument(skip_all)]
pub async fn handle_bid_svm(
    store_new: Arc<StoreNew>,
    bid: BidSvm,
    initiation_time: OffsetDateTime,
    auth: Auth,
) -> result::Result<Uuid, RestError> {
    let store = store_new.store.clone();
    let x = store
        .chains_svm
        .get(&bid.chain_id)
        .ok_or(RestError::InvalidChainId)?;
    let chain_store = x.as_ref();


    let bid_data_svm =
        extract_bid_data_svm(chain_store, bid.transaction.clone(), &chain_store.client).await?;

    let bytes_permission_key = Bytes::from(&bid_data_svm.permission_key.0);
    check_deadline(
        store_new.clone(),
        chain_store,
        &bytes_permission_key,
        bid_data_svm.deadline,
    )
    .await?;
    verify_signatures_svm(
        store_new,
        chain_store,
        &bid,
        &chain_store.express_relay_svm.relayer.pubkey(),
        &bytes_permission_key,
    )
    .await?;
    // TODO we should verify that the wallet bids also include another instruction to the swap program with the appropriate accounts and fields
    simulate_bid_svm(chain_store, &bid).await?;
    // let ssim = Simulator::new(x.clone());
    // ssim.run(bid.transaction.clone()).await;
    //
    // // Check if the bid is not duplicate
    // let bids = chain_store.get_bids(&bytes_permission_key).await;
    // if bids.iter().any(|b| bid == *b) {
    //     return Err(RestError::BadParameters("Duplicate bid".to_string()));
    // }

    let core_fields = SimulatedBidCoreFields::new(bid.chain_id, initiation_time, auth);
    let simulated_bid = SimulatedBidSvm {
        status:         BidStatusSvm::Pending,
        core_fields:    core_fields.clone(),
        transaction:    bid.transaction,
        bid_amount:     bid_data_svm.amount,
        permission_key: bid_data_svm.permission_key,
    };
    store.add_bid(chain_store, simulated_bid).await?;
    Ok(core_fields.id)
}

fn all_signature_exists_svm(
    message_bytes: &[u8],
    accounts: &[Pubkey],
    signatures: &[SignatureSvm],
    missing_signers: &[Pubkey],
) -> bool {
    signatures
        .iter()
        .zip(accounts.iter())
        .all(|(signature, pubkey)| {
            signature.verify(pubkey.as_ref(), message_bytes) || missing_signers.contains(pubkey)
        })
}

async fn verify_signatures_svm(
    store_new: Arc<StoreNew>,
    chain_store: &ChainStoreSvm,
    bid: &BidSvm,
    relayer_pubkey: &Pubkey,
    permission_key: &PermissionKey,
) -> Result<(), RestError> {
    let message_bytes = bid.transaction.message.serialize();
    let signatures = bid.transaction.signatures.clone();
    let accounts = bid.transaction.message.static_account_keys();
    let all_signature_exists = match chain_store
        .get_submission_state(store_new.clone(), permission_key)
        .await
    {
        SubmitType::Invalid => {
            // TODO Look at the todo comment in get_quote.rs file in opportunity module
            return Err(RestError::BadParameters(format!(
                "The permission key is not valid for auction anymore: {:?}",
                permission_key
            )));
        }
        SubmitType::SubmitByOther => {
            let opportunities = store_new
                .opportunity_service_svm
                .get_live_opportunities(GetOpportunitiesInput {
                    key: (bid.chain_id.clone(), permission_key.clone()),
                })
                .await;
            opportunities.into_iter().any(|opportunity| {
                let mut missing_signers = opportunity.get_missing_signers();
                missing_signers.push(*relayer_pubkey);
                all_signature_exists_svm(&message_bytes, accounts, &signatures, &missing_signers)
            })
        }
        SubmitType::SubmitByServer => {
            all_signature_exists_svm(&message_bytes, accounts, &signatures, &[*relayer_pubkey])
        }
    };

    if !all_signature_exists {
        Err(RestError::BadParameters("Invalid signatures".to_string()))
    } else {
        Ok(())
    }
}

async fn simulate_bid_svm(chain_store: &ChainStoreSvm, bid: &BidSvm) -> Result<(), RestError> {
    let response = chain_store.simulate_transaction(&bid.transaction).await;
    let result = response.map_err(|e| {
        tracing::error!("Error while simulating bid: {:?}", e);
        RestError::TemporarilyUnavailable
    })?;
    match result.value {
        Err(err) => {
            tracing::error!(
                "Error while simulating bid: {:?}, context: {:?}",
                err,
                result.context
            );
            let msgs = err.meta.logs;
            // msgs.push(err.to_string());
            Err(RestError::SimulationError {
                result: Default::default(),
                reason: msgs.join("\n"),
            })
        }
        Ok(_) => Ok(()),
    }
}

pub enum SubmitType {
    SubmitByServer,
    SubmitByOther,
    Invalid,
}

/// The trait for the chain store to be implemented for each chain type.
/// These functions are chain specific and should be implemented for each chain in order to handle auctions.
#[async_trait]
pub trait ChainStore:
    Deref<Target = ChainStoreCoreFields<Self::SimulatedBid>> + Send + Sync
{
    /// The trigger type for the chain. This is the type that is used to trigger the auction submission and conclusion.
    type Trigger: DebugTrait + Clone;
    /// The trigger stream type when subscribing to new triggers on the ws client for the chain.
    type TriggerStream<'a>: Stream<Item = Self::Trigger> + Unpin + Send + 'a;
    /// The ws client type for the chain.
    type WsClient;
    /// The simulated bid type for the chain.
    type SimulatedBid: SimulatedBidTrait;
    /// The conclusion result type when try to conclude the auction for the chain.
    type ConclusionResult;
    /// The opportunity service chain type for the chain.
    type OpportunityChainType: OpportunityChainType;

    /// The chain type for the chain.
    const CHAIN_TYPE: models::ChainType;
    /// The minimum lifetime for an auction. If any bid for auction is older than this, the auction is ready to be submitted.
    const AUCTION_MINIMUM_LIFETIME: Duration;

    /// Get the ws client for the chain.
    async fn get_ws_client(&self) -> Result<Self::WsClient>;
    /// Get the trigger stream for the ws client to subscribe to new triggers.
    async fn get_trigger_stream<'a>(client: &'a Self::WsClient) -> Result<Self::TriggerStream<'a>>;
    /// Check if the auction is ready to be concluded based on the trigger.
    fn is_ready_to_conclude(trigger: Self::Trigger) -> bool;

    /// Get the name of the chain according to the configuration.
    fn get_name(&self) -> &ChainId;
    /// Get the winner bids for the auction. Sorting bids by bid amount and simulating the bids to determine the winner bids.
    async fn get_winner_bids(
        &self,
        bids: &[Self::SimulatedBid],
        permission_key: Bytes,
    ) -> Result<Vec<Self::SimulatedBid>>;
    /// Submit the bids for the auction on the chain.
    async fn submit_bids(
        &self,
        permission_key: Bytes,
        bids: Vec<Self::SimulatedBid>,
    ) -> Result<<<Self::SimulatedBid as SimulatedBidTrait>::StatusType as BidStatusTrait>::TxHash>;
    /// Get the bid results for the bids submitted for the auction after the transaction is concluded.
    /// Order of the returned BidStatus is as same as the order of the bids.
    async fn get_bid_results(
        &self,
        bids: Vec<Self::SimulatedBid>,
        tx_hash: Vec<u8>,
    ) -> Result<Option<Vec<<Self::SimulatedBid as SimulatedBidTrait>::StatusType>>>;

    /// Check if the auction winner transaction should be submitted on chain for the permission key.
    async fn get_submission_state(
        &self,
        store_new: Arc<StoreNew>,
        permission_key: &Bytes,
    ) -> SubmitType;

    /// Get the opportunity service for the chain.
    fn get_opportunity_service(
        &self,
        store_new: Arc<StoreNew>,
    ) -> Arc<OpportunityService<Self::OpportunityChainType>>;

    async fn get_bids(&self, key: &PermissionKey) -> Vec<Self::SimulatedBid> {
        self.bids.read().await.get(key).cloned().unwrap_or_default()
    }

    async fn add_bid(&self, bid: Self::SimulatedBid) {
        self.bids
            .write()
            .await
            .entry(bid.get_permission_key_as_bytes())
            .or_insert_with(Vec::new)
            .push(bid);
    }

    async fn remove_bid(&self, bid: Self::SimulatedBid) {
        let mut write_guard = self.bids.write().await;
        let key = bid.get_permission_key_as_bytes();
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
        let key = bid.get_permission_key_as_bytes();
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

    /// Return permission keys with at least one pending bid.
    async fn get_permission_keys_for_auction(&self) -> Vec<PermissionKey> {
        self.bids
            .read()
            .await
            .iter()
            .filter(|(_, bids)| {
                bids.iter()
                    .any(|bid| bid.get_status().clone() == models::BidStatus::Pending)
            })
            .map(|(key, _)| key.clone())
            .collect()
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
                    if bid.get_status().clone() == models::BidStatus::Submitted {
                        if let Some(status_tx_hash) = bid.get_status().get_tx_hash() {
                            return <Self::SimulatedBid as SimulatedBidTrait>::StatusType::convert_tx_hash(status_tx_hash)
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

    async fn opportunity_exists(&self, store_new: Arc<StoreNew>, permission_key: &Bytes) -> bool {
        !self
            .get_opportunity_service(store_new)
            .get_live_opportunities(GetOpportunitiesInput {
                key: (self.get_name().clone(), permission_key.clone()),
            })
            .await
            .is_empty()
    }
}

// While we are submitting bids together, increasing this number will have the following effects:
// 1. There will be more gas required for the transaction, which will result in a higher minimum bid amount.
// 2. The transaction size limit will be reduced for each bid.
// 3. Gas consumption limit will decrease for the bid
const TOTAL_BIDS_PER_AUCTION: usize = 3;

#[async_trait]
impl ChainStore for ChainStoreEvm {
    type Trigger = Block<H256>;
    type TriggerStream<'a> = SubscriptionStream<'a, Ws, Block<H256>>;
    type WsClient = Provider<Ws>;
    type SimulatedBid = SimulatedBidEvm;
    type ConclusionResult = TransactionReceipt;
    type OpportunityChainType = OpportunityChainTypeEvm;

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

    fn is_ready_to_conclude(_trigger: Self::Trigger) -> bool {
        true
    }

    fn get_name(&self) -> &ChainId {
        &self.name
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
        bids.sort_by(|a, b| b.bid_amount.cmp(&a.bid_amount));
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

    async fn get_submission_state(
        &self,
        _store_new: Arc<StoreNew>,
        _permission_key: &Bytes,
    ) -> SubmitType {
        SubmitType::SubmitByServer
    }

    fn get_opportunity_service(
        &self,
        store_new: Arc<StoreNew>,
    ) -> Arc<OpportunityService<Self::OpportunityChainType>> {
        store_new.opportunity_service_evm.clone()
    }
}

const BID_MAXIMUM_LIFE_TIME_SVM: Duration = Duration::from_secs(120);

pub fn add_relayer_signature_svm(relayer: Arc<Keypair>, bid: &mut SimulatedBidSvm) {
    let serialized_message = bid.transaction.message.serialize();
    let relayer_signature_pos = bid
        .transaction
        .message
        .static_account_keys()
        .iter()
        .position(|p| p.eq(&relayer.pubkey()))
        .expect("Relayer not found in static account keys");
    bid.transaction.signatures[relayer_signature_pos] = relayer.sign_message(&serialized_message);
}

/// This is to make sure we are not missing any transaction.
/// We run this once every minute (150 slots).
const CONCLUSION_TRIGGER_SLOT_INTERVAL: u64 = 150;

#[async_trait]
impl ChainStore for ChainStoreSvm {
    type Trigger = SlotInfo;
    type TriggerStream<'a> = Pin<Box<dyn Stream<Item = Self::Trigger> + Send + 'a>>;
    type WsClient = PubsubClient;
    type SimulatedBid = SimulatedBidSvm;
    type ConclusionResult = result::Result<(), TransactionError>;
    type OpportunityChainType = OpportunityChainTypeSvm;

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

    fn is_ready_to_conclude(trigger: Self::Trigger) -> bool {
        trigger.slot % CONCLUSION_TRIGGER_SLOT_INTERVAL == 0
    }

    fn get_name(&self) -> &ChainId {
        &self.name
    }

    async fn get_winner_bids(
        &self,
        bids: &[Self::SimulatedBid],
        _permission_key: Bytes,
    ) -> Result<Vec<Self::SimulatedBid>> {
        let mut bids = bids.to_owned();
        bids.sort_by(|a, b| b.bid_amount.cmp(&a.bid_amount));
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
        add_relayer_signature_svm(relayer, &mut bid);
        match self.send_transaction(&bid.transaction).await {
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
        if bids.is_empty() {
            return Ok(Some(vec![]));
        }

        let tx_hash: SignatureSvm = tx_hash
            .try_into()
            .map_err(|_| anyhow!("Invalid svm signature"))?;
        if bids.len() != 1 {
            tracing::warn!(tx_hash = ?tx_hash, bids = ?bids, "multiple bids found for transaction hash");
        }

        let status = self
            .client
            .get_signature_status_with_commitment(&tx_hash, CommitmentConfig::confirmed())
            .await?;

        let status = match status {
            Some(res) => match res {
                Ok(()) => BidStatusSvm::Won { result: tx_hash },
                Err(_) => BidStatusSvm::Lost {
                    result: Some(tx_hash),
                },
            },
            None => {
                // not yet confirmed
                // TODO Use the correct version of the expiration algorithm, which is:
                // the tx is not expired as long as the block hash is still recent.
                // Assuming a certain block time, the two minute threshold is good enough but in some cases, it's not correct.
                if bids[0].initiation_time + BID_MAXIMUM_LIFE_TIME_SVM < OffsetDateTime::now_utc() {
                    // If the bid is older than the maximum lifetime, it means that the block hash is now too old and the transaction is expired.
                    BidStatusSvm::Expired { result: tx_hash }
                } else {
                    return Ok(None);
                }
            }
        };

        Ok(Some(vec![status; bids.len()]))
    }

    async fn get_submission_state(
        &self,
        store_new: Arc<StoreNew>,
        permission_key: &Bytes,
    ) -> SubmitType {
        if permission_key.starts_with(&self.wallet_program_router_account.to_bytes()) {
            if self.opportunity_exists(store_new, permission_key).await {
                SubmitType::SubmitByOther
            } else {
                SubmitType::Invalid
            }
        } else {
            SubmitType::SubmitByServer
        }
    }

    fn get_opportunity_service(
        &self,
        store_new: Arc<StoreNew>,
    ) -> Arc<OpportunityService<Self::OpportunityChainType>> {
        store_new.opportunity_service_svm.clone()
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

pub async fn run_submission_loop<T: ChainStore + 'static>(
    store_new: Arc<StoreNew>,
    chain_store: Arc<T>,
) -> Result<()> {
    tracing::info!(
        chain_id = chain_store.get_name(),
        "Starting transaction submitter..."
    );
    let mut exit_check_interval = tokio::time::interval(EXIT_CHECK_INTERVAL);

    let ws_client = chain_store.get_ws_client().await?;
    let mut stream = T::get_trigger_stream(&ws_client).await?;

    while !SHOULD_EXIT.load(Ordering::Acquire) {
        tokio::select! {
            trigger = stream.next() => {
                let trigger = trigger.ok_or(anyhow!("Trigger stream ended for chain: {}", chain_store.get_name()))?;
                tracing::debug!("New trigger received for {} at {}: {:?}", chain_store.get_name().clone(), OffsetDateTime::now_utc(), trigger);
                store_new.store.task_tracker.spawn(
                    handle_auctions(store_new.clone(), chain_store.clone())
                );

                if T::is_ready_to_conclude(trigger) {
                    store_new.store.task_tracker.spawn(
                        conclude_submitted_auctions(store_new.store.clone(), chain_store.clone())
                    );
                }
            }
            _ = exit_check_interval.tick() => {}
        }
    }
    tracing::info!("Shutting down transaction submitter...");
    Ok(())
}


pub async fn run_log_listener_loop_svm(
    store_new: Arc<StoreNew>,
    chain_store: Arc<ChainStoreSvm>,
) -> Result<()> {
    tracing::info!(
        chain_id = chain_store.get_name(),
        "Starting log listener..."
    );
    let mut exit_check_interval = tokio::time::interval(EXIT_CHECK_INTERVAL);
    let ws_client = chain_store.get_ws_client().await?;
    let (mut stream, _) = ws_client
        .logs_subscribe(
            RpcTransactionLogsFilter::Mentions(vec![chain_store
                .config
                .express_relay_program_id
                .to_string()]),
            RpcTransactionLogsConfig {
                commitment: Some(CommitmentConfig::confirmed()),
            },
        )
        .await?;

    // TODO Handle the case where connection is lost and we need to reconnect
    while !SHOULD_EXIT.load(Ordering::Acquire) {
        tokio::select! {
            rpc_log = stream.next() => {
                match rpc_log {
                    None => return Err(anyhow!("Log trigger stream ended for chain: {}", chain_store.get_name())),
                    Some(rpc_log) => {
                        tracing::debug!("New log trigger received for {} at {}: {:?}", chain_store.get_name(), OffsetDateTime::now_utc(), rpc_log.clone());
                        store_new.store.task_tracker.spawn({
                            let (store, chain_store) = (store_new.store.clone(), chain_store.clone());
                            async move {
                                let submitted_auctions = chain_store.get_submitted_auctions().await;
                                if let Some(auction) = submitted_auctions.iter().find(|auction| {
                                    auction.tx_hash.clone().map(|tx_hash| {
                                        match SignatureSvm::try_from(tx_hash) {
                                            Ok(tx_hash) => tx_hash.to_string() == rpc_log.value.signature,
                                            Err(err) => {
                                                tracing::error!(error = ?err, "Error while converting tx_hash to SignatureSvm");
                                                false
                                            },
                                        }
                                    }).unwrap_or(false)
                                }) {
                                    if let Err(err) = conclude_submitted_auction(store.clone(), chain_store.as_ref(), auction.clone()).await {
                                        tracing::error!(error = ?err, auction = ?auction, "Error while concluding submitted auction");
                                    }
                                }

                            }
                        });
                    }
                }
            }
            _ = exit_check_interval.tick() => {}
        }
    }
    tracing::info!("Shutting down log listener svm...");

    Ok(())
}
