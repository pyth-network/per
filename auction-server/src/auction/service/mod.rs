#[double]
use crate::opportunity::service::Service as OpportunityService;
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
        auction::service::simulator::Simulator,
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
    mockall_double::double,
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
pub mod cancel_bid;
pub mod conclude_auction;
pub mod get_auction_by_id;
pub mod get_bid;
pub mod get_bids;
pub mod get_express_relay_program_id;
pub mod get_pending_bids;
pub mod get_permission_keys_for_auction;
pub mod handle_auction;
pub mod handle_auctions;
pub mod handle_bid;
pub mod optimize_bids;
pub mod simulator;
pub mod submit_quote;
pub mod update_bid_status;
pub mod update_recent_prioritization_fee;
pub mod verification;
pub mod workers;

pub struct SwapInstructionAccountPositions {
    pub searcher_account:       usize,
    pub router_token_account:   usize,
    pub user_wallet_account:    usize,
    pub mint_searcher_account:  usize,
    pub mint_user_account:      usize,
    pub token_program_searcher: usize,
    pub token_program_user:     usize,
}

pub struct SubmitBidInstructionAccountPositions {
    pub permission_account: usize,
    pub router_account:     usize,
}

pub struct ExpressRelaySvm {
    pub program_id:                               Pubkey,
    pub relayer:                                  Keypair,
    pub submit_bid_instruction_account_positions: SubmitBidInstructionAccountPositions,
    pub swap_instruction_account_positions:       SwapInstructionAccountPositions,
}

pub struct ConfigSvm {
    pub client:                        RpcClient,
    pub express_relay:                 ExpressRelaySvm,
    pub simulator:                     Simulator,
    pub ws_address:                    String,
    pub tx_broadcaster_clients:        Vec<RpcClient>,
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
    opportunity_service: Arc<OpportunityService<T::OpportunityServiceType>>,
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
        opportunity_service: Arc<OpportunityService<T::OpportunityServiceType>>,
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

#[cfg(test)]
pub use mock_service::MockServiceInner as StatefulMockAuctionService;

#[derive(Clone)]
pub struct MockService<T: ChainTrait>(pub Arc<StatefulMockAuctionService<T>>);

impl<T: ChainTrait> MockService<T> {
    pub fn new(mock: StatefulMockAuctionService<T>) -> Self {
        Self(Arc::new(mock))
    }
}

impl<T: ChainTrait> std::ops::Deref for MockService<T> {
    type Target = StatefulMockAuctionService<T>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(test)]
mod mock_service {
    use {
        super::*,
        crate::{
            api::RestError,
            kernel::{
                contracts::{
                    MulticallData,
                    MulticallStatus,
                },
                entities::PermissionKey,
            },
        },
        ethers::contract::FunctionCall,
        mockall::mock,
        solana_sdk::{
            instruction::CompiledInstruction,
            transaction::VersionedTransaction,
        },
    };

    mock! {
        pub ServiceInner<T: ChainTrait> {
            pub fn new(
                db: DB,
                config: Config<T::ConfigType>,
                opportunity_service: Arc<OpportunityService<T::OpportunityServiceType>>,
                task_tracker: TaskTracker,
                event_sender: broadcast::Sender<UpdateEvent>,
            ) -> Self;

            pub fn get_express_relay_program_id(&self) -> Pubkey;

            pub async fn add_auction(
                &self,
                input: add_auction::AddAuctionInput<T>,
            ) -> Result<entities::Auction<T>, RestError>;

            pub async fn cancel_bid_for_lock(
                &self,
                input: cancel_bid::CancelBidInput,
                lock: entities::BidLock,
            ) -> Result<(), RestError>;

            pub async fn conclude_auction_with_statuses(
                &self,
                input: conclude_auction::ConcludeAuctionWithStatusesInput<T>,
            ) -> anyhow::Result<()>;

            pub async fn get_pending_bids(
                &self,
                input: get_pending_bids::GetLiveBidsInput<T::BidChainDataType>,
            ) -> Vec<entities::Bid<T>>;

            pub fn extract_express_relay_instruction(
                &self,
                transaction: VersionedTransaction,
                instruction_type: entities::BidPaymentInstructionType,
            ) -> Result<(usize, CompiledInstruction), RestError> ;

            pub async fn update_bid_status(
                &self,
                input: update_bid_status::UpdateBidStatusInput<T>,
            ) -> Result<bool, RestError>;

            pub async fn handle_bid(
                &self,
                input: handle_bid::HandleBidInput<T>,
            ) -> Result<entities::Bid<T>, RestError>;

            pub fn get_simulation_call(
                &self,
                permission_key: PermissionKey,
                multicall_data: Vec<MulticallData>,
            ) -> FunctionCall<Arc<Provider<TracedClient>>, Provider<TracedClient>, Vec<MulticallStatus>>;

            pub async fn sign_bid_and_submit_auction(
                &self,
                bid: entities::Bid<Svm>,
                auction: entities::Auction<Svm>,
            ) -> Result<VersionedTransaction, RestError>;

            pub fn extract_swap_data(
                instruction: &CompiledInstruction,
            ) -> Result<express_relay::SwapArgs, RestError>;

            pub fn get_new_status(
                bid: &entities::Bid<Svm>,
                submitted_bids: &[entities::Bid<Svm>],
                bid_status_auction: entities::BidStatusAuction<entities::BidStatusSvm>,
            ) -> entities::BidStatusSvm;
        }

        impl<T: ChainTrait> Clone for ServiceInner<T> {
            fn clone(&self) -> Self;
        }
    }
}

#[cfg(test)]
pub mod tests {
    use {
        super::{
            simulator::Simulator,
            Config,
            ConfigSvm,
            ExpressRelaySvm,
            Service,
            ServiceInner,
        },
        crate::{
            auction::repository::{
                Database,
                Repository,
            },
            kernel::{
                entities::{
                    ChainId,
                    Svm,
                },
                traced_sender_svm::{
                    tests::MockRpcClient,
                    TracedSenderSvm,
                },
            },
            opportunity::service::{
                ChainTypeSvm,
                MockService as MockOpportunityService,
            },
            server::{
                get_submit_bid_instruction_account_positions,
                get_swap_instruction_account_positions,
            },
        },
        solana_client::{
            nonblocking::rpc_client::RpcClient,
            rpc_client::RpcClientConfig,
        },
        solana_sdk::signature::Keypair,
        std::sync::Arc,
        tokio::sync::broadcast,
        tokio_util::task::TaskTracker,
    };

    impl Service<Svm> {
        pub fn new_with_mocks_svm(
            chain_id: ChainId,
            db: impl Database<Svm>,
            opportunity_service: MockOpportunityService<ChainTypeSvm>,
            rpc_client: MockRpcClient,
            broadcaster_client: MockRpcClient,
        ) -> Self {
            Service::<Svm>(Arc::new(ServiceInner::<Svm> {
                opportunity_service: Arc::new(opportunity_service),
                config:              Config {
                    chain_id:     chain_id.clone(),
                    chain_config: ConfigSvm {
                        client:                        RpcClient::new_sender(
                            rpc_client,
                            RpcClientConfig::default(),
                        ),
                        express_relay:                 ExpressRelaySvm {
                            program_id: express_relay::id(),

                            relayer:                                  Keypair::new(),
                            submit_bid_instruction_account_positions:
                                get_submit_bid_instruction_account_positions(),
                            swap_instruction_account_positions:
                                get_swap_instruction_account_positions(),
                        },
                        simulator:                     Simulator::new(TracedSenderSvm::new_client(
                            chain_id.clone(),
                            "https://test",
                            2,
                            RpcClientConfig::default(),
                        )),
                        ws_address:                    "ws://test".to_string(),
                        tx_broadcaster_clients:        vec![RpcClient::new_sender(
                            broadcaster_client,
                            RpcClientConfig::default(),
                        )],
                        log_sender:                    broadcast::channel(1).0,
                        prioritization_fee_percentile: None,
                    },
                },
                repo:                Arc::new(Repository::new(db, chain_id.clone())),
                task_tracker:        TaskTracker::new(),
                event_sender:        broadcast::channel(1).0,
            }))
        }
    }
}
