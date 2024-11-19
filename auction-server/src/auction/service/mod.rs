use {
    super::{
        entities,
        repository::{
            self,
            Repository,
        },
    },
    crate::{
        api::ws::UpdateEvent,
        kernel::{
            contracts::{
                LegacyTxTransformer,
                SignableExpressRelayContract,
            },
            db::DB,
            entities::{
                ChainId,
                Evm,
                Svm,
            },
            traced_client::TracedClient,
        },
        opportunity::service as opportunity_service,
    },
    ethers::{
        core::k256::ecdsa::SigningKey,
        middleware::{
            gas_oracle::GasOracleMiddleware,
            NonceManagerMiddleware,
            SignerMiddleware,
            TransformerMiddleware,
        },
        providers::Provider,
        signers::{
            LocalWallet,
            Signer,
            Wallet,
        },
        types::{
            Address,
            U256,
        },
    },
    gas_oracle::EthProviderOracle,
    solana_client::{
        nonblocking::rpc_client::RpcClient,
        rpc_response::{
            Response,
            RpcLogsResponse,
        },
    },
    solana_sdk::{
        pubkey::Pubkey,
        signature::Keypair,
    },
    std::{
        fmt::Debug,
        sync::Arc,
    },
    tokio::sync::broadcast::{
        self,
        Sender,
    },
    tokio_util::task::TaskTracker,
};

pub mod add_auction;
pub mod auction_manager;
pub mod conclude_auction;
pub mod conclude_auctions;
pub mod get_bid;
pub mod get_bids;
pub mod get_live_bids;
pub mod get_permission_keys_for_auction;
pub mod handle_auction;
pub mod handle_auctions;
pub mod handle_bid;
pub mod update_bid_status;
pub mod update_recent_prioritization_fee;
pub mod update_submitted_auction;
pub mod verification;
pub mod workers;

pub struct ExpressRelaySvm {
    pub program_id:                  Pubkey,
    pub relayer:                     Keypair,
    pub permission_account_position: usize,
    pub router_account_position:     usize,
}

pub struct ConfigSvm {
    pub client:                        RpcClient,
    pub wallet_program_router_account: Pubkey,
    pub express_relay:                 ExpressRelaySvm,
    pub ws_address:                    String,
    pub tx_broadcaster_client:         RpcClient,
    pub log_sender:                    Sender<Response<RpcLogsResponse>>,
    pub prioritization_fee_percentile: Option<u64>,
}

pub struct ExpressRelayEvm {
    pub contract_address: Address,
    pub relayer:          Wallet<SigningKey>,
    pub contract:         SignableExpressRelayContract,
}

type GasOracleType = EthProviderOracle<Provider<TracedClient>>;
pub struct ConfigEvm {
    pub express_relay:   ExpressRelayEvm,
    pub provider:        Provider<TracedClient>,
    pub block_gas_limit: U256,
    pub oracle:          GasOracleType,
    pub ws_address:      String,
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

impl ConfigEvm {
    pub fn new(
        relayer: Wallet<SigningKey>,
        contract_address: Address,
        provider: Provider<TracedClient>,
        block_gas_limit: U256,
        ws_address: String,
        network_id: u64,
    ) -> Self {
        Self {
            express_relay: ExpressRelayEvm {
                contract_address,
                contract: get_express_relay_contract(
                    contract_address,
                    provider.clone(),
                    relayer.clone(),
                    false,
                    network_id,
                ),
                relayer,
            },
            block_gas_limit,
            oracle: GasOracleType::new(provider.clone()),
            provider,
            ws_address,
        }
    }
}

pub struct Config<T> {
    pub chain_id: ChainId,

    pub chain_config: T,
}

pub trait ChainTrait:
    Sync + Send + 'static + Debug + Clone + PartialEq + repository::ModelTrait<Self>
{
    type ConfigType: Send + Sync;
    type OpportunityServiceType: opportunity_service::ChainType;

    type BidStatusType: entities::BidStatus;
    type BidChainDataType: entities::BidChainData;
    type BidAmountType: Send + Sync + Debug + Clone + PartialEq;
    type BidChainDataCreateType: Clone + Debug + Send + Sync;

    type ChainStore: Send + Sync + Default + Debug;
}

impl ChainTrait for Evm {
    type ConfigType = ConfigEvm;
    type OpportunityServiceType = opportunity_service::ChainTypeEvm;

    type BidStatusType = entities::BidStatusEvm;
    type BidChainDataType = entities::BidChainDataEvm;
    type BidAmountType = entities::BidAmountEvm;
    type BidChainDataCreateType = entities::BidChainDataCreateEvm;

    type ChainStore = repository::ChainStoreEvm;
}

impl ChainTrait for Svm {
    type ConfigType = ConfigSvm;
    type OpportunityServiceType = opportunity_service::ChainTypeSvm;

    type BidStatusType = entities::BidStatusSvm;
    type BidChainDataType = entities::BidChainDataSvm;
    type BidAmountType = entities::BidAmountSvm;
    type BidChainDataCreateType = entities::BidChainDataCreateSvm;

    type ChainStore = repository::ChainStoreSvm;
}

pub struct ServiceInner<T: ChainTrait> {
    opportunity_service: Arc<opportunity_service::Service<T::OpportunityServiceType>>,
    config:              Config<T::ConfigType>,
    repo:                Arc<Repository<T>>,
    task_tracker:        TaskTracker,
    event_sender:        broadcast::Sender<UpdateEvent>,
}

#[derive(Clone)]
pub struct Service<T: ChainTrait>(Arc<ServiceInner<T>>);
impl<T: ChainTrait> std::ops::Deref for Service<T> {
    type Target = ServiceInner<T>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: ChainTrait> Service<T> {
    pub fn new(
        db: DB,
        config: Config<T::ConfigType>,
        opportunity_service: Arc<opportunity_service::Service<T::OpportunityServiceType>>,
        task_tracker: TaskTracker,
        event_sender: broadcast::Sender<UpdateEvent>,
    ) -> Self {
        Self(Arc::new(ServiceInner {
            repo: Arc::new(repository::Repository::new(db, config.chain_id.clone())),
            config,
            opportunity_service,
            task_tracker,
            event_sender,
        }))
    }
}

#[derive(Clone)]
pub enum ServiceEnum {
    Evm(Service<Evm>),
    Svm(Service<Svm>),
}
