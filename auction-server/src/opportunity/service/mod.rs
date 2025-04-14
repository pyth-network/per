#[double]
use crate::auction::service::Service as AuctionService;
use {
    super::repository::{
        Database,
        InMemoryStore,
        InMemoryStoreSvm,
        Repository,
    },
    crate::{
        auction::service::{
            self as auction_service,
        },
        kernel::{
            entities::{
                ChainId,
                ChainType as ChainTypeEnum,
            },
            traced_sender_svm::TracedSenderSvm,
        },
        state::{
            ChainStoreSvm,
            Store,
        },
    },
    arc_swap::ArcSwap,
    mockall_double::double,
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
pub mod remove_invalid_or_expired_opportunities;
pub mod remove_opportunities;
pub mod verification;

mod get_express_relay_metadata;
mod get_quote_request_account_balances;
mod get_token_program;
mod unwrap_referral_fee_info;

/// Store for the injectable auction service
pub struct AuctionServiceContainer {
    service: ArcSwap<Option<AuctionService>>,
}

impl AuctionServiceContainer {
    pub fn new() -> Self {
        Self {
            service: ArcSwap::new(Arc::new(None)),
        }
    }

    #[allow(unused_variables)]
    pub fn inject_service(&self, service: auction_service::Service) {
        #[cfg(not(test))]
        {
            self.service.swap(Arc::new(Some(service)));
        }

        #[cfg(test)]
        {
            panic!("inject_service should not be called in tests");
        }
    }

    #[cfg(test)]
    pub fn inject_mock_service(&self, service: AuctionService) {
        self.service.swap(Arc::new(Some(service)));
    }

    /// Resolve the stored service
    fn get_service(&self) -> AuctionService {
        self.service
            .load()
            .as_ref()
            .as_ref()
            .expect("no injected service")
            .clone()
    }
}

// NOTE: Do not implement debug here. it has a circular reference to auction_service
pub struct ConfigSvm {
    pub rpc_client:                RpcClient,
    pub accepted_token_programs:   Vec<Pubkey>,
    pub ordered_fee_tokens:        Vec<Pubkey>,
    pub auction_service_container: AuctionServiceContainer,
}

#[allow(dead_code)]
pub trait Config: Send + Sync {}

impl Config for ConfigSvm {
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
                        rpc_client:                TracedSenderSvm::new_client(
                            chain_id.clone(),
                            chain_store.config.rpc_read_url.as_str(),
                            chain_store.config.rpc_timeout,
                            RpcClientConfig::with_commitment(CommitmentConfig::processed()),
                        ),
                        accepted_token_programs:   chain_store
                            .config
                            .accepted_token_programs
                            .clone(),
                        ordered_fee_tokens:        chain_store.config.ordered_fee_tokens.clone(),
                        auction_service_container: AuctionServiceContainer::new(),
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

pub struct ChainTypeSvm;

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
        },
        tokio::sync::{
            broadcast::Receiver,
            RwLock,
        },
    };

    impl Service<ChainTypeSvm> {
        pub fn new_with_mocks_svm(
            chain_id: ChainId,
            db: MockDatabase<InMemoryStoreSvm>,
            rpc_client: MockRpcClient,
        ) -> (Self, Receiver<UpdateEvent>) {
            let config_svm = crate::opportunity::service::ConfigSvm {
                rpc_client:                RpcClient::new_sender(
                    rpc_client,
                    RpcClientConfig::default(),
                ),
                accepted_token_programs:   vec![],
                ordered_fee_tokens:        vec![],
                auction_service_container: AuctionServiceContainer::new(),
            };

            let mut chains_svm = HashMap::new();
            chains_svm.insert(chain_id.clone(), config_svm);

            let store = Arc::new(Store {
                db:            DB::connect_lazy("https://test").unwrap(),
                chains_evm:    HashMap::new(),
                chains_svm:    HashMap::new(),
                ws:            ws::WsState::new("X-Forwarded-For".to_string(), 100),
                secret_key:    "test".to_string(),
                access_tokens: RwLock::new(HashMap::new()),
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
    }
    impl<T: ChainType + 'static> verification::Verification<T> for Service<T> {
        async fn verify_opportunity(
            &self,
            input: verification::VerifyOpportunityInput<<<T::InMemoryStore as InMemoryStore>::Opportunity as crate::opportunity::entities::Opportunity>::OpportunityCreate>,
        ) -> Result<crate::opportunity::entities::OpportunityVerificationResult, crate::api::RestError>;
    }
}
