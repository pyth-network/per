use {
    super::entities,
    crate::kernel::{
        analytics_db::ClickhouseInserter,
        entities::ChainId,
    },
    axum_prometheus::metrics,
    dashmap::DashMap,
    solana_sdk::pubkey::Pubkey,
    std::collections::{
        HashMap,
        VecDeque,
    },
    time::OffsetDateTime,
    tokio::sync::{
        Mutex,
        RwLock,
    },
};

mod add_auction;
mod add_bid;
mod add_bid_analytics;
mod add_lookup_table;
mod add_recent_prioritization_fee;
mod conclude_auction;
mod get_bid;
mod get_bids;
mod get_in_memory_auction_by_bid_id;
mod get_in_memory_auction_by_id;
mod get_in_memory_auctions;
mod get_in_memory_pending_bids;
mod get_in_memory_pending_bids_by_permission_key;
mod get_lookup_table;
mod get_or_create_in_memory_auction_lock;
mod get_or_create_in_memory_bid_lock;
mod get_priority_fees;
mod models;
mod remove_in_memory_auction;
mod remove_in_memory_auction_lock;
mod remove_in_memory_bid_lock;
mod remove_in_memory_pending_bids;
mod submit_auction;
mod update_bid_status;
mod update_in_memory_auction;

use crate::kernel::entities::PermissionKeySvm;
pub use models::*;

#[derive(Debug, Default)]
pub struct ChainStoreSvm {
    lookup_table:               RwLock<HashMap<Pubkey, Vec<Pubkey>>>,
    recent_prioritization_fees: RwLock<VecDeque<PrioritizationFeeSample>>,
}

pub type MicroLamports = u64;
#[derive(Clone, Debug)]
pub struct PrioritizationFeeSample {
    ///micro-lamports per compute unit.
    pub fee:         MicroLamports,
    pub sample_time: OffsetDateTime,
}

#[derive(Debug, Default)]
pub struct ChainStoreEvm {}

#[derive(Debug)]
pub struct InMemoryStore {
    pub pending_bids: RwLock<HashMap<PermissionKeySvm, Vec<entities::Bid>>>,
    pub auctions:     DashMap<entities::AuctionId, entities::Auction>,

    pub auction_lock: Mutex<HashMap<PermissionKeySvm, entities::AuctionLock>>,
    pub bid_lock:     Mutex<HashMap<entities::BidId, entities::BidLock>>,

    pub chain_store: ChainStoreSvm,
}

impl Default for InMemoryStore {
    fn default() -> Self {
        Self {
            pending_bids: RwLock::new(HashMap::new()),
            auctions:     DashMap::new(),
            auction_lock: Mutex::new(HashMap::new()),
            bid_lock:     Mutex::new(HashMap::new()),
            chain_store:  ChainStoreSvm::default(),
        }
    }
}

#[derive(Debug)]
pub struct Repository {
    pub in_memory_store: InMemoryStore,
    pub db:              Box<dyn models::Database>,
    pub db_analytics:    Box<dyn AnalyticsDatabase>,
    pub chain_id:        ChainId,
}

impl Repository {
    pub fn new(
        db: impl models::Database,
        db_analytics: impl models::AnalyticsDatabase,
        chain_id: ChainId,
    ) -> Self {
        Self {
            in_memory_store: InMemoryStore::default(),
            db: Box::new(db),
            db_analytics: Box::new(db_analytics),
            chain_id,
        }
    }
    pub(super) async fn update_metrics(&self) {
        let label = [("chain_id", self.chain_id.to_string())];
        let store = &self.in_memory_store;
        metrics::gauge!("in_memory_auctions", &label).set(store.auctions.len() as f64);
        metrics::gauge!("in_memory_pending_bids", &label)
            .set(store.pending_bids.read().await.len() as f64);
        metrics::gauge!("in_memory_auction_locks", &label)
            .set(store.auction_lock.lock().await.len() as f64);
        metrics::gauge!("in_memory_bid_locks", &label)
            .set(store.bid_lock.lock().await.len() as f64);
    }
}

#[derive(Debug)]
pub struct AnalyticsDatabaseInserter {
    inserter_bid_swap: ClickhouseInserter<BidAnalyticsSwap>,
    inserter_bid_limo: ClickhouseInserter<BidAnalyticsLimo>,
}

impl AnalyticsDatabaseInserter {
    pub fn new(client: clickhouse::Client) -> Self {
        let inserter_bid_swap = ClickhouseInserter::new(client.clone(), "bid_swap".to_string());
        let inserter_bid_limo = ClickhouseInserter::new(client.clone(), "bid_limo".to_string());
        Self {
            inserter_bid_swap,
            inserter_bid_limo,
        }
    }
}
