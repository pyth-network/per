#[double]
use crate::opportunity::service::Service as OpportunityService;
use {
    super::repository::{
        self,
        AnalyticsDatabase,
        AnalyticsDatabaseInserter,
        Repository,
    },
    crate::{
        api::ws::UpdateEvent,
        auction::service::simulator::Simulator,
        kernel::{
            db::DB,
            entities::ChainId,
        },
        state::Store,
    },
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
    std::sync::Arc,
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
pub mod get_bid_transaction_data;
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

pub struct Config {
    pub chain_id: ChainId,

    pub chain_config: ConfigSvm,
}

pub struct ServiceInner {
    store:               Arc<Store>,
    opportunity_service: Arc<OpportunityService>,
    config:              Config,
    repo:                Arc<Repository>,
    task_tracker:        TaskTracker,
    event_sender:        broadcast::Sender<UpdateEvent>,
}

#[derive(Clone)]
pub struct Service(Arc<ServiceInner>);
impl std::ops::Deref for Service {
    type Target = ServiceInner;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub fn create_analytics_db_inserter(client: clickhouse::Client) -> AnalyticsDatabaseInserter {
    AnalyticsDatabaseInserter::new(client)
}

impl Service {
    pub fn new(
        store: Arc<Store>,
        db: DB,
        db_analytics: impl AnalyticsDatabase,
        config: Config,
        opportunity_service: Arc<OpportunityService>,
        task_tracker: TaskTracker,
        event_sender: broadcast::Sender<UpdateEvent>,
    ) -> Self {
        Self(Arc::new(ServiceInner {
            store,
            repo: Arc::new(repository::Repository::new(
                db,
                db_analytics,
                config.chain_id.clone(),
            )),
            config,
            opportunity_service,
            task_tracker,
            event_sender,
        }))
    }
}

#[derive(Clone)]
pub enum ServiceEnum {
    Svm(Service),
}

#[cfg(test)]
pub use {
    mock_service::MockService,
    mock_service::MockServiceInner as StatefulMockAuctionService,
};

#[cfg(test)]
mod mock_service {
    use {
        super::*,
        crate::{
            api::RestError,
            auction::entities,
        },
        mockall::mock,
        solana_sdk::transaction::VersionedTransaction,
    };

    #[derive(Clone)]
    pub struct MockService(pub Arc<StatefulMockAuctionService>);

    impl MockService {
        pub fn new(mock: StatefulMockAuctionService) -> Self {
            Self(Arc::new(mock))
        }
    }

    impl std::ops::Deref for MockService {
        type Target = StatefulMockAuctionService;
        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    mock! {
        pub ServiceInner {
            pub fn new(
                db: DB,
                config: Config,
                opportunity_service: Arc<OpportunityService>,
                task_tracker: TaskTracker,
                event_sender: broadcast::Sender<UpdateEvent>,
            ) -> Self;

            pub fn get_express_relay_program_id(&self) -> Pubkey;

            pub async fn add_auction(
                &self,
                input: add_auction::AddAuctionInput,
            ) -> Result<entities::Auction, RestError>;

            pub async fn cancel_bid_for_lock(
                &self,
                input: cancel_bid::CancelBidInput,
                lock: entities::BidLock,
            ) -> Result<(), RestError>;

            pub async fn conclude_auction_with_statuses(
                &self,
                input: conclude_auction::ConcludeAuctionWithStatusesInput,
            ) -> anyhow::Result<()>;

            pub async fn get_pending_bids(
                &self,
                input: get_pending_bids::GetLiveBidsInput,
            ) -> Vec<entities::Bid>;

            pub async fn update_bid_status(
                &self,
                input: update_bid_status::UpdateBidStatusInput,
            ) -> Result<bool, RestError>;

            pub async fn handle_bid(
                &self,
                input: handle_bid::HandleBidInput,
            ) -> Result<entities::Bid, RestError>;

            pub async fn sign_bid_and_submit_auction(
                &self,
                bid: entities::Bid,
                auction: entities::Auction,
            ) -> Result<VersionedTransaction, RestError>;

            pub fn get_new_status(
                bid: &entities::Bid,
                submitted_bids: &[entities::Bid],
                bid_status_auction: entities::BidStatusAuction,
            ) -> entities::BidStatusSvm;

            pub async fn get_bid_transaction_data_swap(
                &self,
                transaction: VersionedTransaction,
            ) -> Result<entities::BidTransactionDataSwap, RestError>;
        }

        impl Clone for ServiceInner {
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
            api::ws,
            auction::repository::{
                Database,
                MockAnalyticsDatabase,
                Repository,
            },
            kernel::{
                db::DB,
                entities::ChainId,
                traced_sender_svm::{
                    tests::MockRpcClient,
                    TracedSenderSvm,
                },
            },
            opportunity::service::MockService as MockOpportunityService,
            server::{
                get_submit_bid_instruction_account_positions,
                get_swap_instruction_account_positions,
            },
            state::Store,
        },
        solana_client::{
            nonblocking::rpc_client::RpcClient,
            rpc_client::RpcClientConfig,
        },
        solana_sdk::signature::Keypair,
        std::{
            collections::HashMap,
            sync::Arc,
        },
        tokio::sync::{
            broadcast,
            RwLock,
        },
        tokio_util::task::TaskTracker,
    };

    impl Service {
        pub fn new_with_mocks_svm(
            chain_id: ChainId,
            db: impl Database,
            opportunity_service: MockOpportunityService,
            rpc_client: MockRpcClient,
            broadcaster_client: MockRpcClient,
        ) -> Self {
            let store = Arc::new(Store {
                db:            DB::connect_lazy("https://test").unwrap(),
                chains_svm:    HashMap::new(),
                ws:            ws::WsState::new("X-Forwarded-For".to_string(), 100),
                secret_key:    "test".to_string(),
                access_tokens: RwLock::new(HashMap::new()),
                privileges:    RwLock::new(HashMap::new()),
                prices:        RwLock::new(HashMap::new()),
            });
            Service(Arc::new(ServiceInner {
                store,
                opportunity_service: Arc::new(opportunity_service),
                config: Config {
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
                repo: Arc::new(Repository::new(
                    db,
                    MockAnalyticsDatabase::new(),
                    chain_id.clone(),
                )),
                task_tracker: TaskTracker::new(),
                event_sender: broadcast::channel(1).0,
            }))
        }
    }
}
