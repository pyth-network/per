use {
    crate::{
        api::RestError,
        config::EthereumConfig,
        server::{
            EXIT_CHECK_INTERVAL,
            SHOULD_EXIT,
        },
        state::{
            BidStatus,
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
            FunctionCall,
        },
        middleware::{
            transformer::{
                Transformer,
                TransformerError,
            },
            SignerMiddleware,
            TransformerMiddleware,
        },
        providers::{
            Http,
            Provider,
            ProviderError,
        },
        signers::{
            LocalWallet,
            Signer,
        },
        types::{
            transaction::eip2718::TypedTransaction,
            Address,
            Bytes,
            TransactionReceipt,
            TransactionRequest,
            U256,
        },
    },
    serde::{
        Deserialize,
        Serialize,
    },
    std::{
        result,
        sync::{
            atomic::Ordering,
            Arc,
        },
        time::Duration,
    },
    utoipa::ToSchema,
    uuid::Uuid,
};

abigen!(
    PER,
    "../per_multicall/out/PERMulticall.sol/PERMulticall.json"
);
pub type PERContract = PER<Provider<Http>>;
pub type SignableProvider =
    TransformerMiddleware<SignerMiddleware<Provider<Http>, LocalWallet>, LegacyTxTransformer>;
pub type SignablePERContract = PER<SignableProvider>;

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

pub fn get_simulation_call(
    per_operator: Address,
    provider: Provider<Http>,
    chain_config: EthereumConfig,
    permission: Bytes,
    contracts: Vec<Address>,
    calldata: Vec<Bytes>,
    bids: Vec<U256>,
) -> FunctionCall<Arc<Provider<Http>>, Provider<Http>, Vec<per::MulticallStatus>> {
    let client = Arc::new(provider);
    let per_contract = PERContract::new(chain_config.per_contract, client);

    per_contract
        .multicall(permission, contracts, calldata, bids)
        .from(per_operator)
}


pub enum SimulationError {
    LogicalError { result: Bytes, reason: String },
    ContractError(ContractError<Provider<Http>>),
}


pub fn evaluate_simulation_results(
    results: Vec<per::MulticallStatus>,
) -> Result<(), SimulationError> {
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
    per_operator: Address,
    provider: Provider<Http>,
    chain_config: EthereumConfig,
    permission: Bytes,
    contracts: Vec<Address>,
    calldata: Vec<Bytes>,
    bids: Vec<U256>,
) -> Result<(), SimulationError> {
    let call = get_simulation_call(
        per_operator,
        provider,
        chain_config,
        permission,
        contracts,
        calldata,
        bids,
    );
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
    contracts: Vec<Address>,
    calldata: Vec<Bytes>,
    bids: Vec<U256>,
) -> Result<Option<TransactionReceipt>, SubmissionError> {
    let transformer = LegacyTxTransformer {
        use_legacy_tx: chain_config.legacy_tx,
    };
    let client = Arc::new(TransformerMiddleware::new(
        SignerMiddleware::new(provider, signer_wallet.with_chain_id(network_id)),
        transformer,
    ));

    let per_contract = SignablePERContract::new(chain_config.per_contract, client);
    let call = per_contract.multicall(permission, contracts, calldata, bids);
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
                    tracing::info!(
                        "Chain: {chain_id} Auctions to process {auction_len}",
                        chain_id = chain_id,
                        auction_len = chain_store.bids.len()
                    );
                    for (permission_key, mut cloned_bids) in chain_store.bids.clone().into_iter() {
                         cloned_bids.sort_by(|a, b| b.bid.cmp(&a.bid));

                        // TODO: simulate all bids together and keep the successful ones
                        // keep the highest bid for now
                        let winner_bids = &cloned_bids[..1].to_vec();
                        let submission = submit_bids(
                            store.per_operator.clone(),
                            chain_store.provider.clone(),
                            chain_store.config.clone(),
                            chain_store.network_id,
                            permission_key.clone(),
                            winner_bids.iter().map(|b| b.contract).collect(),
                            winner_bids.iter().map(|b| b.calldata.clone()).collect(),
                            winner_bids.iter().map(|b| b.bid).collect(),
                        )
                        .await;
                        match submission {
                            Ok(receipt) => match receipt {
                                Some(receipt) => {
                                    tracing::debug!("Submitted transaction: {:?}", receipt);
                                    let winner_ids:Vec<Uuid> = winner_bids.iter().map(|b| b.id).collect();
                                    for bid in &cloned_bids {
                                        let status = match winner_ids.contains(&bid.id) {
                                            true => BidStatus::Submitted(receipt.transaction_hash),
                                            false => BidStatus::Lost
                                        };
                                        store.bid_status_store.set_and_broadcast(bid.id, status).await;
                                    }
                                    chain_store.bids.remove_if_mut(&permission_key, |_, bids| {
                                        bids.retain(|b| !&cloned_bids.contains(b));
                                        bids.is_empty()
                                    });
                                }
                                None => {
                                    tracing::error!("Failed to receive transaction receipt");
                                }
                            },
                            Err(err) => {
                                //TODO: handle error, remove invalid bids, set bid status, etc
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
    #[schema(example = "0xdeadbeef", value_type=String)]
    pub permission_key: Bytes,
    /// The chain id to bid on.
    #[schema(example = "sepolia", value_type=String)]
    pub chain_id:       String,
    /// The contract address to call.
    #[schema(example = "0xcA11bde05977b3631167028862bE2a173976CA11",value_type = String)]
    pub contract:       abi::Address,
    /// Calldata for the contract call.
    #[schema(example = "0xdeadbeef", value_type=String)]
    pub calldata:       Bytes,
    /// Amount of bid in wei.
    #[schema(example = "10", value_type=String)]
    #[serde(with = "crate::serde::u256")]
    pub amount:         U256,
}

pub async fn handle_bid(store: Arc<Store>, bid: Bid) -> result::Result<Uuid, RestError> {
    let chain_store = store
        .chains
        .get(&bid.chain_id)
        .ok_or(RestError::InvalidChainId)?;
    let call = simulate_bids(
        store.per_operator.address(),
        chain_store.provider.clone(),
        chain_store.config.clone(),
        bid.permission_key.clone(),
        vec![bid.contract],
        vec![bid.calldata.clone()],
        vec![bid.amount],
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
    chain_store
        .bids
        .entry(bid.permission_key.clone())
        .or_default()
        .push(SimulatedBid {
            contract: bid.contract,
            calldata: bid.calldata.clone(),
            bid:      bid.amount,
            id:       bid_id,
        });
    store
        .bid_status_store
        .set_and_broadcast(bid_id, BidStatus::Pending)
        .await;
    Ok(bid_id)
}
