#![allow(dead_code)]
#![allow(unused_imports)]

use {
    super::entities,
    crate::kernel::{
        db::DB,
        entities::{
            ChainId,
            Evm,
            Svm,
        },
    },
    solana_sdk::pubkey::Pubkey,
    std::collections::HashMap,
    tokio::sync::{
        Mutex,
        RwLock,
    },
};

mod add_bid;
mod add_lookup_table;
mod get_bid;
mod get_bids;
mod get_lookup_table;
mod models;

pub use models::*;
pub const BID_PAGE_SIZE_CAP: usize = 100;

type PermissionKey<T> =
    <<T as entities::BidTrait>::ChainData as entities::BidChainData>::PermissionKey;

#[derive(Debug, Default)]
pub struct ChainStoreSvm {
    lookup_table: RwLock<HashMap<Pubkey, Vec<Pubkey>>>,
}

#[derive(Debug, Default)]
pub struct ChainStoreEvm {}

pub trait InMemoryStoreTrait: entities::BidTrait {
    type ChainStore: Default + std::fmt::Debug;
}

impl InMemoryStoreTrait for Evm {
    type ChainStore = ChainStoreEvm;
}

impl InMemoryStoreTrait for Svm {
    type ChainStore = ChainStoreSvm;
}

#[derive(Debug)]
pub struct InMemoryStore<T: InMemoryStoreTrait> {
    pub bids:               RwLock<HashMap<PermissionKey<T>, Vec<entities::Bid<T>>>>,
    pub auction_lock:       Mutex<HashMap<PermissionKey<T>, entities::AuctionLock>>,
    pub submitted_auctions: RwLock<Vec<models::Auction>>,

    pub chain_store: T::ChainStore,
}

impl<T: InMemoryStoreTrait> Default for InMemoryStore<T> {
    fn default() -> Self {
        Self {
            bids:               RwLock::new(HashMap::new()),
            auction_lock:       Mutex::new(HashMap::new()),
            submitted_auctions: RwLock::new(Vec::new()),
            chain_store:        T::ChainStore::default(),
        }
    }
}

pub trait RepositoryTrait: InMemoryStoreTrait + models::BidTrait {}
impl RepositoryTrait for Evm {
}
impl RepositoryTrait for Svm {
}

#[derive(Debug)]
pub struct Repository<T: RepositoryTrait> {
    pub in_memory_store: InMemoryStore<T>,
    pub db:              DB,
    pub chain_id:        ChainId,
}

impl<T: RepositoryTrait> Repository<T> {
    pub fn new(db: DB, chain_id: ChainId) -> Self {
        Self {
            in_memory_store: InMemoryStore::default(),
            db,
            chain_id,
        }
    }
}
