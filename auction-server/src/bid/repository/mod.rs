#![allow(dead_code)]
#![allow(unused_imports)]

use {
    super::entities,
    crate::kernel::{
        db::DB,
        entities::ChainId,
    },
    std::collections::HashMap,
    tokio::sync::{
        Mutex,
        RwLock,
    },
};

pub mod get_bids;
mod models;

pub use models::*;
pub const BID_PAGE_SIZE_CAP: usize = 100;

type PermissionKey<T> =
    <<T as entities::BidTrait>::ChainData as entities::BidChainData>::PermissionKey;

#[derive(Debug)]
pub struct InMemoryStore<T: entities::BidTrait> {
    pub bids:               RwLock<HashMap<PermissionKey<T>, Vec<entities::Bid<T>>>>,
    pub auction_lock:       Mutex<HashMap<PermissionKey<T>, entities::AuctionLock>>,
    pub submitted_auctions: RwLock<Vec<models::Auction>>,
}

impl<T: entities::BidTrait> Default for InMemoryStore<T> {
    fn default() -> Self {
        Self {
            bids:               RwLock::new(HashMap::new()),
            auction_lock:       Mutex::new(HashMap::new()),
            submitted_auctions: RwLock::new(Vec::new()),
        }
    }
}

#[derive(Debug)]
pub struct Repository<T: entities::BidTrait> {
    pub in_memory_store: InMemoryStore<T>,
    pub db:              DB,
    pub chain_id:        ChainId,
}

impl<T: entities::BidTrait> Repository<T> {
    pub fn new(db: DB, chain_id: ChainId) -> Self {
        Self {
            in_memory_store: InMemoryStore::<T>::default(),
            db,
            chain_id,
        }
    }
}
