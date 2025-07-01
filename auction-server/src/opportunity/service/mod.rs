#[double]
use crate::auction::service::Service as AuctionService;
use {
    super::repository::{
        AnalyticsDatabase,
        Database,
        Repository,
    },
    crate::{
        api::RestError,
        auction::service::{
            self as auction_service,
        },
        config::{
            MinimumPlatformFeeListConfig,
            MinimumReferralFeeListConfig,
            TokenWhitelistConfig,
        },
        kernel::{
            entities::ChainId,
            traced_sender_svm::TracedSenderSvm,
        },
        opportunity::repository::AnalyticsDatabaseInserter,
        per_metrics::QUOTE_VALIDATION_TOTAL,
        state::{
            ChainStoreSvm,
            Store,
        },
    },
    arc_swap::ArcSwap,
    axum_prometheus::metrics,
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
        cmp::max,
        collections::HashMap,
        sync::Arc,
        time::Duration,
    },
    tokio_util::task::TaskTracker,
    uuid::Uuid,
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
pub mod get_token_mint;
pub mod remove_invalid_or_expired_opportunities;
pub mod remove_opportunities;
pub mod remove_opportunity;

mod add_opportunity_analytics;
mod get_quote_request_account_balances;
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
    pub rpc_client:                          RpcClient,
    pub accepted_token_programs:             Vec<Pubkey>,
    pub ordered_fee_tokens:                  Vec<Pubkey>,
    pub auction_service_container:           AuctionServiceContainer,
    pub token_whitelist:                     TokenWhitelist,
    pub minimum_referral_fee_list:           MinimumReferralFeeList,
    pub minimum_platform_fee_list:           Vec<MinimumFee>,
    pub allow_permissionless_quote_requests: bool,
    pub auction_time:                        Duration,
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
                        rpc_client:                          TracedSenderSvm::new_client(
                            chain_id.clone(),
                            chain_store.config.rpc_read_url.as_str(),
                            chain_store.config.rpc_timeout,
                            RpcClientConfig::with_commitment(CommitmentConfig::processed()),
                        ),
                        accepted_token_programs:             chain_store
                            .config
                            .accepted_token_programs
                            .clone(),
                        ordered_fee_tokens:                  chain_store
                            .config
                            .ordered_fee_tokens
                            .clone(),
                        auction_service_container:           AuctionServiceContainer::new(),
                        token_whitelist:                     chain_store
                            .config
                            .token_whitelist
                            .clone()
                            .into(),
                        minimum_referral_fee_list:           chain_store
                            .config
                            .minimum_referral_fee_list
                            .clone()
                            .into(),
                        minimum_platform_fee_list:           chain_store
                            .config
                            .minimum_platform_fee_list
                            .clone()
                            .into(),
                        allow_permissionless_quote_requests: chain_store
                            .config
                            .allow_permissionless_quote_requests,
                        auction_time:                        chain_store.config.auction_time,
                    },
                )
            })
            .collect())
    }

    pub fn validate_quote(
        &self,
        chain_id: ChainId,
        mint_user: Pubkey,
        mint_searcher: Pubkey,
        profile_id: Option<Uuid>,
        referral_fee_ppm: u64,
    ) -> Result<(), RestError> {
        let profile_id_label = profile_id.map_or("None".to_string(), |id| id.to_string());
        if !self.token_whitelist.is_token_mint_allowed(&mint_user) {
            metrics::counter!(
                QUOTE_VALIDATION_TOTAL,
                &[
                    ("chain_id", chain_id),
                    ("profile_id", profile_id_label),
                    ("result", "invalid_user_mint_not_allowed".to_string()),
                ]
            )
            .increment(1);
            return Err(RestError::TokenMintNotAllowed(
                "Input".to_string(),
                mint_user.to_string(),
            ));
        }
        if !self.token_whitelist.is_token_mint_allowed(&mint_searcher) {
            metrics::counter!(
                QUOTE_VALIDATION_TOTAL,
                &[
                    ("chain_id", chain_id),
                    ("profile_id", profile_id_label),
                    ("result", "invalid_searcher_mint_not_allowed".to_string()),
                ]
            )
            .increment(1);
            return Err(RestError::TokenMintNotAllowed(
                "Output".to_string(),
                mint_searcher.to_string(),
            ));
        }

        if !self.allow_permissionless_quote_requests & profile_id.is_none() {
            metrics::counter!(
                QUOTE_VALIDATION_TOTAL,
                &[
                    ("chain_id", chain_id),
                    ("profile_id", profile_id_label),
                    ("result", "invalid_unauthorized".to_string()),
                ]
            )
            .increment(1);
            return Err(RestError::Unauthorized);
        }

        let minimum_fee_searcher = self
            .minimum_referral_fee_list
            .get_minimum_fee(&mint_searcher, profile_id);
        let minimum_fee_user = self
            .minimum_referral_fee_list
            .get_minimum_fee(&mint_user, profile_id);

        if referral_fee_ppm
            < max(
                minimum_fee_searcher.unwrap_or(0),
                minimum_fee_user.unwrap_or(0),
            )
        {
            metrics::counter!(
                QUOTE_VALIDATION_TOTAL,
                &[
                    ("chain_id", chain_id),
                    ("profile_id", profile_id_label),
                    ("result", "invalid_referral_fee_below_minimum".to_string()),
                ]
            )
            .increment(1);
            return Err(RestError::QuoteNotFound);
        }

        metrics::counter!(
            QUOTE_VALIDATION_TOTAL,
            &[
                ("chain_id", chain_id),
                ("profile_id", profile_id_label),
                ("result", "valid".to_string()),
            ]
        )
        .increment(1);

        Ok(())
    }

    pub fn get_platform_fee_ppm(&self, mint_user: &Pubkey, mint_searcher: &Pubkey) -> Option<u64> {
        let fee_user = self.minimum_platform_fee_list.iter().find_map(|fee| {
            if &fee.mint == mint_user {
                Some(fee.fee_ppm)
            } else {
                None
            }
        });
        let fee_searcher = self.minimum_platform_fee_list.iter().find_map(|fee| {
            if &fee.mint == mint_searcher {
                Some(fee.fee_ppm)
            } else {
                None
            }
        });

        match (fee_user, fee_searcher) {
            (Some(user_fee), Some(searcher_fee)) => Some(max(user_fee, searcher_fee)),
            (Some(user_fee), None) => Some(user_fee),
            (None, Some(searcher_fee)) => Some(searcher_fee),
            (None, None) => None,
        }
    }
}

/// Optional minimum referral fee list for token mints
#[derive(Clone, Default)]
pub struct MinimumReferralFeeList {
    pub profiles: Vec<MinimumFeeProfile>,
}

#[derive(Clone, Default)]
pub struct MinimumFeeProfile {
    pub profile_id:   Option<Uuid>,
    pub minimum_fees: Vec<MinimumFee>,
}

#[derive(Clone, Default)]
pub struct MinimumFee {
    pub mint:    Pubkey,
    pub fee_ppm: u64,
}

impl MinimumReferralFeeList {
    pub fn get_minimum_fee(&self, mint: &Pubkey, profile_id: Option<Uuid>) -> Option<u64> {
        let mut minimum_fee = self
            .profiles
            .iter()
            .find(|profile| profile.profile_id == profile_id)
            .and_then(|profile| {
                profile
                    .minimum_fees
                    .iter()
                    .find(|fee| &fee.mint == mint)
                    .map(|fee| fee.fee_ppm)
            });

        // The minimum referral fee list can include an entry with no profile_id, which can be used as a fallback if no match is found for the specific profile_id.
        // This allows for a default minimum referral fee to be applied if no specific profile is found.
        if minimum_fee.is_none() {
            minimum_fee = self
                .profiles
                .iter()
                .find(|profile| profile.profile_id.is_none())
                .and_then(|profile| {
                    profile
                        .minimum_fees
                        .iter()
                        .find(|fee| &fee.mint == mint)
                        .map(|fee| fee.fee_ppm)
                });
        }

        minimum_fee
    }
}

impl From<MinimumReferralFeeListConfig> for MinimumReferralFeeList {
    fn from(value: MinimumReferralFeeListConfig) -> Self {
        Self {
            profiles: value
                .profiles
                .into_iter()
                .map(|profile| MinimumFeeProfile {
                    profile_id:   profile.profile_id,
                    minimum_fees: profile
                        .minimum_fees
                        .into_iter()
                        .map(|fee| MinimumFee {
                            mint:    fee.mint,
                            fee_ppm: fee.fee_ppm,
                        })
                        .collect(),
                })
                .collect(),
        }
    }
}

impl From<MinimumPlatformFeeListConfig> for Vec<MinimumFee> {
    fn from(value: MinimumPlatformFeeListConfig) -> Self {
        value
            .minimum_fees
            .into_iter()
            .map(|fee| MinimumFee {
                mint:    fee.mint,
                fee_ppm: fee.fee_ppm,
            })
            .collect()
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
                rpc_client:                          rpc_tester.make_test_client(),
                accepted_token_programs:             vec![],
                ordered_fee_tokens:                  vec![],
                auction_service_container:           AuctionServiceContainer::new(),
                token_whitelist:                     Default::default(),
                minimum_referral_fee_list:           Default::default(),
                minimum_platform_fee_list:           Default::default(),
                allow_permissionless_quote_requests: true,
                auction_time:                        config::ConfigSvm::default_auction_time(),
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
                prices:        RwLock::new(HashMap::new()),
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
        pub async fn get_token_mint(
            &self,
            input: crate::opportunity::service::get_token_mint::GetTokenMintInput,
        ) -> Result<crate::opportunity::entities::TokenMint, crate::api::RestError>;
    }
}
