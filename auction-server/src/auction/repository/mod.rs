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
    std::collections::HashMap,
    tokio::sync::{
        Mutex,
        RwLock,
    },
};

mod add_auction;
mod add_bid;
mod add_lookup_table;
mod conclude_auction;
mod get_bid;
mod get_bids;
mod get_in_memory_bids;
mod get_in_memory_bids_by_permission_key;
mod get_in_memory_submitted_auctions;
mod get_in_memory_submitted_bids_for_auction;
mod get_lookup_table;
mod get_or_create_in_memory_auction_lock;
mod models;
mod remove_in_memory_auction_lock;
mod remove_in_memory_submitted_auction;
mod submit_auction;
mod update_bid_status;

pub use models::*;

#[derive(Debug, Default)]
pub struct ChainStoreSvm {
    lookup_table: RwLock<HashMap<Pubkey, Vec<Pubkey>>>,
}

#[derive(Debug, Default)]
pub struct ChainStoreEvm {}

#[derive(Debug)]
pub struct InMemoryStore<T: ChainTrait> {
    pub bids:               RwLock<HashMap<entities::PermissionKey<T>, Vec<entities::Bid<T>>>>,
    pub auction_lock:       Mutex<HashMap<entities::PermissionKey<T>, entities::AuctionLock>>,
    pub submitted_auctions: RwLock<Vec<entities::Auction<T>>>,

    pub chain_store: T::ChainStore,
}

impl<T: ChainTrait> Default for InMemoryStore<T> {
    fn default() -> Self {
        Self {
            bids:               RwLock::new(HashMap::new()),
            auction_lock:       Mutex::new(HashMap::new()),
            submitted_auctions: RwLock::new(Vec::new()),
            chain_store:        T::ChainStore::default(),
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
