use {
    super::entities,
    ethers::types::Address,
    std::{
        collections::HashMap,
        ops::Deref,
    },
    tokio::sync::RwLock,
};

mod add_opportunity;
mod add_spoof_info;
mod exists_in_memory_opportunity_create;
mod get_in_memory_opportunities;
mod get_in_memory_opportunities_by_key;
mod get_in_memory_opportunity_by_id;
mod get_opportunities;
mod get_spoof_info;
mod models;
mod remove_opportunities;
mod remove_opportunity;

pub use models::*;
pub const OPPORTUNITY_PAGE_SIZE_CAP: i32 = 100;

#[derive(Debug)]
pub struct Repository<T: InMemoryStore> {
    pub in_memory_store: T,
}

pub trait InMemoryStore:
    Deref<Target = InMemoryStoreCoreFields<Self::Opportunity>> + Send + Sync
{
    type Opportunity: entities::Opportunity;

    fn new() -> Self;
}

pub struct InMemoryStoreCoreFields<T: entities::Opportunity> {
    pub opportunities: RwLock<HashMap<entities::OpportunityKey, Vec<T>>>,
}

impl<T: entities::Opportunity> InMemoryStoreCoreFields<T> {
    pub fn new() -> Self {
        Self {
            opportunities: RwLock::new(HashMap::new()),
        }
    }
}

pub struct InMemoryStoreEvm {
    pub core_fields: InMemoryStoreCoreFields<entities::OpportunityEvm>,
    pub spoof_info:  RwLock<HashMap<Address, entities::SpoofState>>,
}
pub struct InMemoryStoreSvm {
    pub core_fields: InMemoryStoreCoreFields<entities::OpportunitySvm>,
}

impl InMemoryStore for InMemoryStoreEvm {
    type Opportunity = entities::OpportunityEvm;

    fn new() -> Self {
        Self {
            core_fields: InMemoryStoreCoreFields::new(),
            spoof_info:  RwLock::new(HashMap::new()),
        }
    }
}

impl InMemoryStore for InMemoryStoreSvm {
    type Opportunity = entities::OpportunitySvm;

    fn new() -> Self {
        Self {
            core_fields: InMemoryStoreCoreFields::new(),
        }
    }
}

impl Deref for InMemoryStoreEvm {
    type Target = InMemoryStoreCoreFields<entities::OpportunityEvm>;

    fn deref(&self) -> &Self::Target {
        &self.core_fields
    }
}

impl Deref for InMemoryStoreSvm {
    type Target = InMemoryStoreCoreFields<entities::OpportunitySvm>;

    fn deref(&self) -> &Self::Target {
        &self.core_fields
    }
}

impl<T: InMemoryStore> Repository<T> {
    pub fn new() -> Self {
        Self {
            in_memory_store: T::new(),
        }
    }
}
