use {
    crate::{
        api::RestError,
        config::{ChainId, EthereumConfig},
        server::{EXIT_CHECK_INTERVAL, SHOULD_EXIT},
        state::{
            AuctionParams, AuctionParamsWithMetadata, BidAmount, BidStatus, BidStatusWithId,
            PermissionKey, SimulatedBid, Store,
        },
    },
    anyhow::{anyhow, Result},
    ethers::{
        abi,
        contract::{abigen, ContractError, EthError, FunctionCall},
        middleware::{
            transformer::{Transformer, TransformerError},
            SignerMiddleware, TransformerMiddleware,
        },
        providers::{Http, Provider, ProviderError},
        signers::{LocalWallet, Signer},
        types::{
            transaction::eip2718::TypedTransaction, Address, Bytes, TransactionReceipt,
            TransactionRequest, H160, U256,
        },
    },
    serde::{Deserialize, Serialize},
    sqlx::types::time::OffsetDateTime,
    std::{
        collections::HashMap,
        result,
        sync::{atomic::Ordering, Arc},
        time::Duration,
    },
    utoipa::ToSchema,
    uuid::Uuid,
};

abigen!(
    ExpressRelay,
    "../per_multicall/out/ExpressRelay.sol/ExpressRelay.json"
);
pub type ExpressRelayContract = ExpressRelay<Provider<Http>>;
pub type SignableProvider =
    TransformerMiddleware<SignerMiddleware<Provider<Http>, LocalWallet>, LegacyTxTransformer>;
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
            bid_id: x.0,
            target_contract: x.1,
            target_calldata: x.2,
            bid_amount: x.3,
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
}

pub enum SimulationError {
    LogicalError { result: Bytes, reason: String },
    ContractError(ContractError<Provider<Http>>),
}

pub fn evaluate_simulation_results(results: Vec<MulticallStatus>) -> Result<(), SimulationError> {
    let failed_result = results.iter().find(|x| !x.external_success);
    if let Some(call_status) = failed_result {
        return Err(SimulationError::LogicalError {
            result: call_status.external_result.clone(),
            reason: call_status.multicall_revert_reason.clone(),
        });
    }
    Ok(())
}

pub async fn simulate_bids(
    relayer: Address,
    provider: Provider<Http>,
    chain_config: EthereumConfig,
    permission: Bytes,
    multicall_data: Vec<MulticallData>,
) -> Result<(), SimulationError> {
    let call = get_simulation_call(relayer, provider, chain_config, permission, multicall_data);
    match call.await {
        Ok(results) => {
            evaluate_simulation_results(results)?;
        }
        Err(e) => {
            return Err(SimulationError::ContractError(e));
        }
    };
    Ok(())
}

#[derive(Debug)]
pub enum SubmissionError {
    ProviderError(ProviderError),
    ContractError(ContractError<SignableProvider>),
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
    signer_wallet: LocalWallet,
    provider: Provider<Http>,
    chain_config: EthereumConfig,
    network_id: u64,
    permission: Bytes,
    multicall_data: Vec<MulticallData>,
) -> Result<Option<TransactionReceipt>, SubmissionError> {
    let transformer = LegacyTxTransformer {
        use_legacy_tx: chain_config.legacy_tx,
    };
    let client = Arc::new(TransformerMiddleware::new(
        SignerMiddleware::new(provider, signer_wallet.with_chain_id(network_id)),
        transformer,
    ));

    let express_relay_contract =
        SignableExpressRelayContract::new(chain_config.express_relay_contract, client);

    let call = express_relay_contract.multicall(permission, multicall_data);
    let mut gas_estimate = call
        .estimate_gas()
        .await
        .map_err(SubmissionError::ContractError)?;
    let gas_multiplier = U256::from(2); //TODO: smarter gas estimation
    gas_estimate *= gas_multiplier;
    let call_with_gas = call.gas(gas_estimate);
    let send_call = call_with_gas
        .send()
        .await
        .map_err(SubmissionError::ContractError)?;

    send_call.await.map_err(SubmissionError::ProviderError)
}

pub async fn run_submission_loop(store: Arc<Store>) -> Result<()> {
    tracing::info!("Starting transaction submitter...");
    let mut exit_check_interval = tokio::time::interval(EXIT_CHECK_INTERVAL);

    // this should be replaced by a subscription to the chain and trigger on new blocks
    let mut submission_interval = tokio::time::interval(Duration::from_secs(5));
    while !SHOULD_EXIT.load(Ordering::Acquire) {
        tokio::select! {
            _ = submission_interval.tick() => {
                for (chain_id, chain_store) in &store.chains {
                    let all_bids = store.get_bids_by_chain_id(chain_id).await;
                    let bid_by_permission_key:HashMap<PermissionKey,Vec<SimulatedBid>> =
                    all_bids.into_iter().fold(HashMap::new(),
                        |mut acc, bid| {
                        acc.entry(bid.permission_key.clone()).or_default().push(bid);
                        acc
                    });

                    tracing::info!(
                        "Chain: {chain_id} Auctions to process {auction_len}",
                        chain_id = chain_id,
                        auction_len = bid_by_permission_key.len()
                    );
                    for (permission_key, bids) in bid_by_permission_key.iter() {
                        let auction_id = store.init_auction(permission_key.clone(), chain_id.clone()).await?;

                        let mut cloned_bids = bids.clone();
                        let permission_key = permission_key.clone();
                         cloned_bids.sort_by(|a, b| b.bid_amount.cmp(&a.bid_amount));

                        // TODO: simulate all bids together and keep the successful ones
                        // keep the highest bid for now
                        let winner_bids = &cloned_bids[..1].to_vec();
                        let submission = submit_bids(
                            store.relayer.clone(),
                            chain_store.provider.clone(),
                            chain_store.config.clone(),
                            chain_store.network_id,
                            permission_key.clone(),
                            winner_bids.iter().map(|b| MulticallData::from((b.id.to_bytes_le(), b.target_contract, b.target_calldata.clone(), b.bid_amount))).collect()
                        )
                        .await;
                        match submission {
                            Ok(receipt) => match receipt {
                                Some(receipt) => {
                                    tracing::debug!("Submitted transaction: {:?}", receipt);
                                    let auction_params = AuctionParams {
                                        chain_id: chain_id.clone(),
                                        permission_key: permission_key.clone(),
                                        tx_hash: receipt.transaction_hash,
                                    };
                                    let auction = AuctionParamsWithMetadata {
                                        id: auction_id,
                                        conclusion_time: OffsetDateTime::now_utc().unix_timestamp_nanos() / 1000,
                                        params: auction_params,
                                    };
                                    store.update_auction(auction).await?;
                                    let winner_ids:Vec<Uuid> = winner_bids.iter().map(|b| b.id).collect();
                                    for bid in cloned_bids {
                                        let bid_index = winner_ids.iter().position(|&x| x == bid.id);
                                        let bid_status = match bid_index {
                                            Some(i) => BidStatus::Submitted { result: receipt.transaction_hash, index: i as u32 },
                                            None => BidStatus::Lost { result: receipt.transaction_hash }
                                        };
                                        store.broadcast_bid_status_and_remove(BidStatusWithId { id: bid.id, bid_status }, auction_id).await?;
                                    }
                                }
                                None => {
                                    tracing::error!("Failed to receive transaction receipt");
                                }
                            },
                            Err(err) => {
                                tracing::error!("Transaction failed to submit: {:?}", err);
                            }
                        }

                    }
                }
            }
            _ = exit_check_interval.tick() => {}
        }
    }
    tracing::info!("Shutting down transaction submitter...");
    Ok(())
}

#[derive(Serialize, Deserialize, ToSchema, Clone, Debug)]
pub struct Bid {
    /// The permission key to bid on.
    #[schema(example = "0xdeadbeef", value_type = String)]
    pub permission_key: Bytes,
    /// The chain id to bid on.
    #[schema(example = "op_sepolia", value_type = String)]
    pub chain_id: ChainId,
    /// The contract address to call.
    #[schema(example = "0xcA11bde05977b3631167028862bE2a173976CA11", value_type = String)]
    pub target_contract: abi::Address,
    /// Calldata for the contract call.
    #[schema(example = "0xdeadbeef", value_type = String)]
    pub target_calldata: Bytes,
    /// Amount of bid in wei.
    #[schema(example = "10", value_type = String)]
    #[serde(with = "crate::serde::u256")]
    pub amount: BidAmount,
}

pub async fn handle_bid(store: Arc<Store>, bid: Bid) -> result::Result<Uuid, RestError> {
    let chain_store = store
        .chains
        .get(&bid.chain_id)
        .ok_or(RestError::InvalidChainId)?;
    let call = simulate_bids(
        store.relayer.address(),
        chain_store.provider.clone(),
        chain_store.config.clone(),
        bid.permission_key.clone(),
        vec![MulticallData::from((
            Uuid::new_v4().to_bytes_le(),
            bid.target_contract,
            bid.target_calldata.clone(),
            bid.amount,
        ))],
    );

    if let Err(e) = call.await {
        return match e {
            SimulationError::LogicalError { result, reason } => {
                Err(RestError::SimulationError { result, reason })
            }
            SimulationError::ContractError(e) => match e {
                ContractError::Revert(reason) => Err(RestError::BadParameters(format!(
                    "Contract Revert Error: {}",
                    String::decode_with_selector(&reason)
                        .unwrap_or("unable to decode revert".to_string())
                ))),
                ContractError::MiddlewareError { e: _ } => Err(RestError::TemporarilyUnavailable),
                ContractError::ProviderError { e: _ } => Err(RestError::TemporarilyUnavailable),
                _ => Err(RestError::BadParameters(format!("Error: {}", e))),
            },
        };
    };

    let bid_id = Uuid::new_v4();
    let simulated_bid = SimulatedBid {
        target_contract: bid.target_contract,
        target_calldata: bid.target_calldata.clone(),
        bid_amount: bid.amount,
        id: bid_id,
        permission_key: bid.permission_key.clone(),
        chain_id: bid.chain_id.clone(),
        status: BidStatus::Pending,
    };
    store.add_bid(simulated_bid).await?;
    Ok(bid_id)
}
