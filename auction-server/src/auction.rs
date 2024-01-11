use anyhow::anyhow;
use std::{
    sync::{atomic::Ordering, Arc},
    time::Duration,
};

use ethers::{
    contract::{abigen, ContractError},
    middleware::{
        transformer::{Transformer, TransformerError},
        SignerMiddleware, TransformerMiddleware,
    },
    providers::{Http, Provider, ProviderError},
    signers::{LocalWallet, Signer},
    types::{
        transaction::eip2718::TypedTransaction, Address, Bytes, TransactionReceipt,
        TransactionRequest, U256,
    },
};

use crate::{api::SHOULD_EXIT, config::EthereumConfig, state::Store};

abigen!(PER, "src/PERMulticall.json");
pub type PERContract = PER<Provider<Http>>;
pub type SignableProvider =
    TransformerMiddleware<SignerMiddleware<Provider<Http>, LocalWallet>, LegacyTxTransformer>;
pub type SignablePERContract = PER<SignableProvider>;

impl TryFrom<EthereumConfig> for Provider<Http> {
    fn try_from(config: EthereumConfig) -> Result<Self, Self::Error> {
        Provider::<Http>::try_from(config.geth_rpc_addr.clone()).map_err(|err| {
            anyhow!(
                "Failed to connect to {rpc_addr}: {:?}",
                err,
                rpc_addr = config.geth_rpc_addr
            )
        })
    }
    type Error = anyhow::Error;
}

pub async fn simulate_bids(
    per_operator: Address,
    provider: Provider<Http>,
    chain_config: EthereumConfig,
    permission: Bytes,
    contracts: Vec<Address>,
    calldata: Vec<Bytes>,
    bids: Vec<U256>,
) -> Result<Vec<MulticallStatus>, ContractError<Provider<Http>>> {
    let client = Arc::new(provider);
    let per_contract = PERContract::new(chain_config.contract_addr, client);
    let call = per_contract
        .multicall(permission, contracts, calldata, bids)
        .from(per_operator);
    call.call().await
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

    let per_contract = SignablePERContract::new(chain_config.contract_addr, client);
    let call = per_contract.multicall(permission, contracts, calldata, bids);
    let send_call = call.send().await.map_err(SubmissionError::ContractError)?;
    send_call.await.map_err(SubmissionError::ProviderError)
}

pub async fn run_submission_loop(store: Arc<Store>) {
    tracing::info!("Starting transaction submitter...");
    while !SHOULD_EXIT.load(Ordering::Acquire) {
        for (chain_id, chain_store) in &store.chains {
            let permission_bids = chain_store.bids.read().await;
            tracing::info!(
                "Chain: {chain_id} Auctions to process {auction_len}",
                chain_id = chain_id,
                auction_len = permission_bids.len()
            );
            tracing::info!(
                "Chain store: {n} contracts' vaults surfaced",
                n = (chain_store.opps.write().await.keys().len())
            );
            for (permission_key, bids) in permission_bids.iter() {
                let mut cloned_bids = bids.clone();
                let thread_store = store.clone();
                let chain_id = chain_id.clone();
                let permission_key = permission_key.clone();
                tokio::spawn(async move {
                    cloned_bids.sort_by(|a, b| b.bid.cmp(&a.bid));

                    // TODO: simulate all bids together and keep the successful ones
                    // let call = simulate_bids(
                    //     store.per_operator.address(),
                    //     chain_store.contract_addr,
                    //     chain_store.provider.clone(),
                    //     permission_key.clone(),
                    //     cloned_bids.iter().map(|b| b.contract).collect(),
                    //     cloned_bids.iter().map(|b| b.calldata.clone()).collect(),
                    //     cloned_bids.iter().map(|b| b.bid.into()).collect(),
                    // );

                    // keep the highest bid for now
                    cloned_bids.truncate(1);

                    match thread_store.chains.get(&chain_id) {
                        Some(chain_store) => {
                            let submission = submit_bids(
                                thread_store.per_operator.clone(),
                                chain_store.provider.clone(),
                                chain_store.config.clone(),
                                chain_store.network_id,
                                permission_key.clone(),
                                cloned_bids.iter().map(|b| b.contract).collect(),
                                cloned_bids.iter().map(|b| b.calldata.clone()).collect(),
                                cloned_bids.iter().map(|b| b.bid.into()).collect(),
                            )
                            .await;
                            match submission {
                                Ok(receipt) => match receipt {
                                    Some(receipt) => {
                                        tracing::info!("Submitted transaction: {:?}", receipt);
                                        chain_store.bids.write().await.remove(&permission_key);
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
                        None => {
                            tracing::error!("Chain not found: {}", chain_id);
                        }
                    }
                });
            }
        }
        tokio::time::sleep(Duration::from_secs(10)).await; // this should be replaced by a subscription to the chain and trigger on new blocks
    }
    tracing::info!("Shutting down transaction submitter...");
}
