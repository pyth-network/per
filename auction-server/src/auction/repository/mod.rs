use {
    super::{
        entities,
        service::ChainTrait,
    },
    crate::kernel::{
        db::DB,
        entities::ChainId,
    },
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
mod add_lookup_table;
mod add_recent_priotization_fee;
mod conclude_auction;
mod get_bid;
mod get_bids;
mod get_in_memory_auction_by_id;
mod get_in_memory_auctions;
mod get_in_memory_pending_bids;
mod get_in_memory_pending_bids_by_permission_key;
mod get_lookup_table;
mod get_or_create_in_memory_auction_lock;
mod get_priority_fees;
mod models;
mod remove_in_memory_auction;
mod remove_in_memory_auction_lock;
mod remove_in_memory_pending_bids;
mod submit_auction;
mod update_bid_status;
mod update_in_memory_auction;

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
pub struct InMemoryStore<T: ChainTrait> {
    pub pending_bids: RwLock<HashMap<entities::PermissionKey<T>, Vec<entities::Bid<T>>>>,
    pub auction_lock: Mutex<HashMap<entities::PermissionKey<T>, entities::AuctionLock>>,
    pub auctions:     RwLock<Vec<entities::Auction<T>>>,

    pub chain_store: T::ChainStore,
}

impl<T: ChainTrait> Default for InMemoryStore<T> {
    fn default() -> Self {
        Self {
            pending_bids: RwLock::new(HashMap::new()),
            auction_lock: Mutex::new(HashMap::new()),
            auctions:     RwLock::new(Vec::new()),
            chain_store:  T::ChainStore::default(),
        }
    }
}

#[derive(Debug)]
pub struct Repository<T: ChainTrait> {
    pub in_memory_store: InMemoryStore<T>,
    pub db:              DB,
    pub chain_id:        ChainId,
}

impl<T: ChainTrait> Repository<T> {
    pub fn new(db: DB, chain_id: ChainId) -> Self {
        Self {
            in_memory_store: InMemoryStore::default(),
            db,
            chain_id,
        }
    }
}
