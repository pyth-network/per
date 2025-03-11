use {
    super::repository::{
        Database,
        InMemoryStore,
        InMemoryStoreEvm,
        InMemoryStoreSvm,
        Repository,
    },
    crate::{
        auction::service::{
            self as auction_service,
        },
        kernel::{
            contracts::AdapterFactory,
            entities::{
                ChainId,
                ChainType as ChainTypeEnum,
                Evm,
                Svm,
            },
            traced_client::TracedClient,
            traced_sender_svm::TracedSenderSvm,
        },
        state::{
            ChainStoreEvm,
            ChainStoreSvm,
            Store,
        },
    },
    ethers::{
        providers::Provider,
        types::Address,
    },
    futures::future::try_join_all,
    solana_client::{
        nonblocking::rpc_client::RpcClient,
        rpc_client::RpcClientConfig,
    },
    solana_sdk::{
        commitment_config::CommitmentConfig,
        pubkey::Pubkey,
    },
    std::{
        collections::HashMap,
        sync::Arc,
    },
    tokio::sync::RwLock,
    tokio_util::task::TaskTracker,
};
#[cfg(test)]
use {
    crate::kernel::db::DB,
    mockall::mock,
};

pub mod add_opportunity;
pub mod get_config;
pub mod get_live_opportunities;
pub mod get_opportunities;
pub mod get_quote;
pub mod handle_opportunity_bid;
pub mod remove_invalid_or_expired_opportunities;
pub mod remove_opportunities;
pub mod verification;

mod get_express_relay_metadata;
mod get_spoof_info;
mod get_token_program;
mod make_adapter_calldata;
mod make_opportunity_execution_params;
mod make_permitted_tokens;
mod unwrap_referral_fee_info;
mod get_quote_request_associated_token_accounts;

// NOTE: Do not implement debug here. it has a circular reference to auction_service
pub struct ConfigEvm {
    pub adapter_factory_contract: Address,
    pub adapter_bytecode_hash:    [u8; 32],
    pub chain_id_num:             u64,
    pub permit2:                  Address,
    pub provider:                 Provider<TracedClient>,
    pub weth:                     Address,
    pub auction_service:          RwLock<Option<auction_service::Service<Evm>>>,
}

impl ConfigEvm {
    // TODO Move these to config trait?
    pub async fn inject_auction_service(&self, service: auction_service::Service<Evm>) {
        let mut write_guard = self.auction_service.write().await;
        *write_guard = Some(service);
    }
    pub async fn get_auction_service(&self) -> auction_service::Service<Evm> {
        self.auction_service
            .read()
            .await
            .clone()
            .expect("Failed to get auction service")
    }
}

// NOTE: Do not implement debug here. it has a circular reference to auction_service
pub struct ConfigSvm {
    pub auction_service:         RwLock<Option<auction_service::Service<Svm>>>,
    pub rpc_client:              RpcClient,
    pub accepted_token_programs: Vec<Pubkey>,
}

impl ConfigSvm {
    // TODO Move these to config trait?
    pub async fn inject_auction_service(&self, service: auction_service::Service<Svm>) {
        let mut write_guard = self.auction_service.write().await;
        *write_guard = Some(service);
    }
    pub async fn get_auction_service(&self) -> auction_service::Service<Svm> {
        self.auction_service
            .read()
            .await
            .clone()
            .expect("Failed to get auction service")
    }
}

#[allow(dead_code)]
pub trait Config: Send + Sync {}

impl Config for ConfigEvm {
}
impl Config for ConfigSvm {
}

impl ConfigEvm {
    async fn get_weth_address(
        adapter_contract: Address,
        provider: Provider<TracedClient>,
    ) -> anyhow::Result<Address> {
        let adapter = AdapterFactory::new(adapter_contract, Arc::new(provider));
        adapter
            .get_weth()
            .call()
            .await
            .map_err(|e| anyhow::anyhow!("Error getting WETH address from adapter: {:?}", e))
    }

    async fn get_adapter_bytecode_hash(
        adapter_contract: Address,
        provider: Provider<TracedClient>,
    ) -> anyhow::Result<[u8; 32]> {
        let adapter = AdapterFactory::new(adapter_contract, Arc::new(provider));
        adapter
            .get_opportunity_adapter_creation_code_hash()
            .call()
            .await
            .map_err(|e| anyhow::anyhow!("Error getting adapter code hash from adapter: {:?}", e))
    }

    async fn get_permit2_address(
        adapter_contract: Address,
        provider: Provider<TracedClient>,
    ) -> anyhow::Result<Address> {
        let adapter = AdapterFactory::new(adapter_contract, Arc::new(provider));
        adapter
            .get_permit_2()
            .call()
            .await
            .map_err(|e| anyhow::anyhow!("Error getting permit2 address from adapter: {:?}", e))
    }

    async fn try_new(
        adapter_factory_contract: Address,
        provider: Provider<TracedClient>,
        chain_id_num: u64,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            adapter_bytecode_hash: Self::get_adapter_bytecode_hash(
                adapter_factory_contract,
                provider.clone(),
            )
            .await?,
            permit2: Self::get_permit2_address(adapter_factory_contract, provider.clone()).await?,
            weth: Self::get_weth_address(adapter_factory_contract, provider.clone()).await?,
            adapter_factory_contract,
            chain_id_num,
            provider,
            auction_service: RwLock::new(None),
        })
    }

    pub async fn from_chains(
        chains: &HashMap<ChainId, ChainStoreEvm>,
    ) -> anyhow::Result<HashMap<ChainId, Self>> {
        let config_opportunity_service_evm = chains.iter().map(|(chain_id, chain_store)| {
            let chain_id_cloned = chain_id.clone();
            let adapter_factory_contract = chain_store.config.adapter_factory_contract;
            let provider_cloned = chain_store.provider.clone();
            async move {
                let config = Self::try_new(
                    adapter_factory_contract,
                    provider_cloned.clone(),
                    chain_store.network_id,
                )
                .await?;
                Ok::<(ChainId, Self), anyhow::Error>((chain_id_cloned, config))
            }
        });
        Ok(try_join_all(config_opportunity_service_evm)
            .await?
            .into_iter()
            .collect())
    }
}

impl ConfigSvm {
    pub async fn from_chains(
        chains: &HashMap<ChainId, ChainStoreSvm>,
    ) -> anyhow::Result<HashMap<ChainId, Self>> {
        Ok(chains
            .iter()
            .map(|(chain_id, chain_store)| {
                (
                    chain_id.clone(),
                    Self {
                        auction_service:         RwLock::new(None),
                        rpc_client:              TracedSenderSvm::new_client(
                            chain_id.clone(),
                            chain_store.config.rpc_read_url.as_str(),
                            chain_store.config.rpc_timeout,
                            RpcClientConfig::with_commitment(CommitmentConfig::processed()),
                        ),
                        accepted_token_programs: chain_store.config.accepted_token_programs.clone(),
                    },
                )
            })
            .collect())
    }
}

pub trait ChainType: Send + Sync {
    type Config: Config;
    type InMemoryStore: InMemoryStore;

    fn get_type() -> ChainTypeEnum;
}

pub struct ChainTypeEvm;
pub struct ChainTypeSvm;

impl ChainType for ChainTypeEvm {
    type Config = ConfigEvm;
    type InMemoryStore = InMemoryStoreEvm;

    fn get_type() -> ChainTypeEnum {
        ChainTypeEnum::Evm
    }
}

impl ChainType for ChainTypeSvm {
    type Config = ConfigSvm;
    type InMemoryStore = InMemoryStoreSvm;

    fn get_type() -> ChainTypeEnum {
        ChainTypeEnum::Svm
    }
}

// TODO maybe just create a service per chain_id?
pub struct Service<T: ChainType> {
    store:        Arc<Store>,
    // TODO maybe after adding state for opportunity we can remove the arc
    repo:         Arc<Repository<T::InMemoryStore>>,
    config:       HashMap<ChainId, T::Config>,
    task_tracker: TaskTracker,
}

impl<T: ChainType> Service<T> {
    pub fn new(
        store: Arc<Store>,
        task_tracker: TaskTracker,
        db: impl Database<T::InMemoryStore>,
        config: HashMap<ChainId, T::Config>,
    ) -> Self {
        Self {
            store,
            repo: Arc::new(Repository::new(db)),
            config,
            task_tracker,
        }
    }
    pub async fn update_metrics(&self) {
        self.repo.update_metrics().await;
    }
}

#[cfg(test)]
pub mod tests {
    use {
        super::*,
        crate::{
            api::ws::{
                self,
                UpdateEvent,
            },
            kernel::traced_sender_svm::tests::MockRpcClient,
            opportunity::repository::MockDatabase,
            server::setup_metrics_recorder,
        },
        std::sync::atomic::AtomicUsize,
        tokio::sync::broadcast::Receiver,
    };

    impl Service<ChainTypeSvm> {
        pub fn new_with_mocks_svm(
            chain_id: ChainId,
            db: MockDatabase<InMemoryStoreSvm>,
            rpc_client: MockRpcClient,
        ) -> (Self, Receiver<UpdateEvent>) {
            let config_svm = crate::opportunity::service::ConfigSvm {
                auction_service:         RwLock::new(None),
                rpc_client:              RpcClient::new_sender(
                    rpc_client,
                    RpcClientConfig::default(),
                ),
                accepted_token_programs: vec![],
            };

            let (broadcast_sender, broadcast_receiver) = tokio::sync::broadcast::channel(100);

            let mut chains_svm = HashMap::new();
            chains_svm.insert(chain_id.clone(), config_svm);

            let store = Arc::new(Store {
                db:               DB::connect_lazy("https://test").unwrap(),
                chains_evm:       HashMap::new(),
                chains_svm:       HashMap::new(),
                ws:               ws::WsState {
                    subscriber_counter: AtomicUsize::new(0),
                    broadcast_sender,
                    broadcast_receiver,
                },
                secret_key:       "test".to_string(),
                access_tokens:    RwLock::new(HashMap::new()),
                metrics_recorder: setup_metrics_recorder().unwrap(),
            });

            let ws_receiver = store.ws.broadcast_receiver.resubscribe();

            let service =
                Service::<ChainTypeSvm>::new(store.clone(), TaskTracker::new(), db, chains_svm);

            (service, ws_receiver)
        }
    }
}

#[cfg(test)]
mock! {
    pub Service<T: ChainType + 'static> {
        pub fn new(
            store: Arc<Store>,
            task_tracker: TaskTracker,
            db: DB,
            config: HashMap<ChainId, T::Config>,
        ) -> Self;
        pub fn get_config(&self, chain_id: &ChainId) -> Result<T::Config, crate::api::RestError>;
        pub async fn get_live_opportunities(&self, input: get_live_opportunities::GetLiveOpportunitiesInput) -> Vec<<T::InMemoryStore as InMemoryStore>::Opportunity>;
        pub async fn get_live_opportunity_by_id(&self, input: get_opportunities::GetLiveOpportunityByIdInput) -> Option<<T::InMemoryStore as InMemoryStore>::Opportunity>;
        pub async fn remove_invalid_or_expired_opportunities(&self);
        pub async fn update_metrics(&self);
        pub async fn remove_opportunities(
            &self,
            input: remove_opportunities::RemoveOpportunitiesInput,
        ) -> Result<(), crate::api::RestError>;
        pub async fn add_opportunity(
            &self,
            input: add_opportunity::AddOpportunityInput<<<T::InMemoryStore as InMemoryStore>::Opportunity as crate::opportunity::entities::Opportunity>::OpportunityCreate>,
        ) -> Result<<T::InMemoryStore as InMemoryStore>::Opportunity, crate::api::RestError>;
        pub async fn get_opportunities(
            &self,
            input: get_opportunities::GetOpportunitiesInput,
        ) -> Result<Vec<<T::InMemoryStore as InMemoryStore>::Opportunity>, crate::api::RestError>;
        pub async fn get_quote(&self, input: get_quote::GetQuoteInput) -> Result<crate::opportunity::entities::Quote, crate::api::RestError>;
        pub async fn handle_opportunity_bid(
            &self,
            input: handle_opportunity_bid::HandleOpportunityBidInput,
        ) -> Result<uuid::Uuid, crate::api::RestError>;
    }
    impl<T: ChainType + 'static> verification::Verification<T> for Service<T> {
        async fn verify_opportunity(
            &self,
            input: verification::VerifyOpportunityInput<<<T::InMemoryStore as InMemoryStore>::Opportunity as crate::opportunity::entities::Opportunity>::OpportunityCreate>,
        ) -> Result<crate::opportunity::entities::OpportunityVerificationResult, crate::api::RestError>;
    }
}
