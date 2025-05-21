#[double]
use crate::auction::service::Service as AuctionService;
use {
    super::repository::{
        AnalyticsDatabase,
        Database,
        Repository,
    },
    crate::{
        auction::service::{
            self as auction_service,
        },
        config::TokenWhitelistConfig,
        kernel::{
            entities::ChainId,
            traced_sender_svm::TracedSenderSvm,
        },
        opportunity::repository::AnalyticsDatabaseInserter,
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
        time::Duration,
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
pub mod get_express_relay_metadata;
pub mod get_live_opportunities;
pub mod get_opportunities;
pub mod get_quote;
pub mod remove_invalid_or_expired_opportunities;
pub mod remove_opportunities;
pub mod remove_opportunity;

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
    pub token_whitelist:           TokenWhitelist,
    pub auction_time:              Duration,
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
                        token_whitelist:           chain_store
                            .config
                            .token_whitelist
                            .clone()
                            .into(),
                        auction_time:              chain_store.config.auction_time,
                    },
                )
            })
            .collect())
    }
}

/// Optional whitelist for token mints
#[derive(Clone, Default)]
pub struct TokenWhitelist {
    pub enabled:         bool,
    pub whitelist_mints: Vec<Pubkey>,
}

impl TokenWhitelist {
    /// Returns true if the token is whitelisted or if the whitelist feature is disabled
    pub fn is_token_mint_allowed(&self, token_mint: &Pubkey) -> bool {
        !self.enabled || self.whitelist_mints.binary_search(token_mint).is_ok()
    }
}

impl From<TokenWhitelistConfig> for TokenWhitelist {
    fn from(value: TokenWhitelistConfig) -> Self {
        let mut whitelist = value.whitelist_mints;
        whitelist.sort();

        Self {
            enabled:         value.enabled,
            whitelist_mints: whitelist,
        }
    }
}

// TODO maybe just create a service per chain_id?
#[derive(Clone)]
pub struct Service(Arc<ServiceInner>);
impl std::ops::Deref for Service {
    type Target = ServiceInner;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct ServiceInner {
    store:        Arc<Store>,
    // TODO maybe after adding state for opportunity we can remove the arc
    repo:         Arc<Repository>,
    config:       HashMap<ChainId, ConfigSvm>,
    task_tracker: TaskTracker,
}

pub fn create_analytics_db_inserter(client: clickhouse::Client) -> AnalyticsDatabaseInserter {
    AnalyticsDatabaseInserter::new(client)
}

impl Service {
    pub fn new(
        store: Arc<Store>,
        task_tracker: TaskTracker,
        db: impl Database,
        db_analytics: impl AnalyticsDatabase,
        config: HashMap<ChainId, ConfigSvm>,
    ) -> Self {
        Self(Arc::new(ServiceInner {
            store,
            repo: Arc::new(Repository::new(db, db_analytics)),
            config,
            task_tracker,
        }))
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
            config,
            kernel::rpc_client_svm_tester::RpcClientSvmTester,
            opportunity::repository::{
                MockAnalyticsDatabase,
                MockDatabase,
            },
        },
        tokio::sync::{
            broadcast::Receiver,
            RwLock,
        },
    };

    impl Service {
        pub fn new_with_mocks_svm(
            chain_id: ChainId,
            db: MockDatabase,
            rpc_tester: &RpcClientSvmTester,
        ) -> (Self, Receiver<UpdateEvent>) {
            let config_svm = crate::opportunity::service::ConfigSvm {
                rpc_client:                rpc_tester.make_test_client(),
                accepted_token_programs:   vec![],
                ordered_fee_tokens:        vec![],
                auction_service_container: AuctionServiceContainer::new(),
                token_whitelist:           Default::default(),
                auction_time:              config::ConfigSvm::default_auction_time(),
            };

            let mut chains_svm = HashMap::new();
            chains_svm.insert(chain_id.clone(), config_svm);

            let store = Arc::new(Store {
                db:            DB::connect_lazy("https://test").unwrap(),
                chains_svm:    HashMap::new(),
                ws:            ws::WsState::new("X-Forwarded-For".to_string(), 100),
                secret_key:    "test".to_string(),
                access_tokens: RwLock::new(HashMap::new()),
                privileges:    RwLock::new(HashMap::new()),
            });

            let ws_receiver = store.ws.broadcast_receiver.resubscribe();

            let service = Service::new(
                store.clone(),
                TaskTracker::new(),
                db,
                MockAnalyticsDatabase::new(),
                chains_svm,
            );

            (service, ws_receiver)
        }
    }
}

#[cfg(test)]
use crate::opportunity::entities::OpportunitySvm;

#[cfg(test)]
mock! {
    pub Service {
        pub fn new(
            store: Arc<Store>,
            task_tracker: TaskTracker,
            db: DB,
            db_analytics: AnalyticsDatabaseInserter,
            config: HashMap<ChainId, ConfigSvm>,
        ) -> Self;
        pub fn get_config(&self, chain_id: &ChainId) -> Result<ConfigSvm, crate::api::RestError>;
        pub async fn get_live_opportunities(&self, input: get_live_opportunities::GetLiveOpportunitiesInput) -> Vec<OpportunitySvm>;
        pub async fn get_live_opportunity_by_id(&self, input: get_opportunities::GetLiveOpportunityByIdInput) -> Option<OpportunitySvm>;
        pub async fn remove_invalid_or_expired_opportunities(&self);
        pub async fn update_metrics(&self);
        pub async fn remove_opportunities(
            &self,
            input: remove_opportunities::RemoveOpportunitiesInput,
        ) -> Result<(), crate::api::RestError>;
        pub async fn add_opportunity(
            &self,
            input: add_opportunity::AddOpportunityInput,
        ) -> Result<OpportunitySvm, crate::api::RestError>;
        pub async fn get_opportunities(
            &self,
            input: get_opportunities::GetOpportunitiesInput,
        ) -> Result<Vec<OpportunitySvm>, crate::api::RestError>;
        pub async fn get_quote(&self, input: get_quote::GetQuoteInput) -> Result<crate::opportunity::entities::Quote, crate::api::RestError>;
        pub async fn get_express_relay_metadata(&self, input: get_express_relay_metadata::GetExpressRelayMetadataInput) -> Result<express_relay::state::ExpressRelayMetadata, crate::api::RestError>;
    }
}
