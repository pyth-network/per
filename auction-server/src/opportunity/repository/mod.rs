use {
    super::entities,
    crate::kernel::entities::PermissionKey,
    ethers::types::Address,
    std::{
        collections::HashMap,
        ops::Deref,
    },
    tokio::sync::RwLock,
};

mod add_opportunity;
mod add_spoof_info;
mod get_live_opportunities_by_permission_key;
mod get_opportunities;
mod get_opportunities_by_permission_key;
mod get_opportunities_by_permission_key_and_id;
mod get_spoof_info;
mod models;
mod opportunity_exists;
mod remove_opportunity;

pub use models::*;
pub const OPPORTUNITY_PAGE_SIZE: i32 = 20;

#[derive(Debug)]
pub struct Repository<T: InMemoryStore> {
    pub in_memory_store: T,
}

pub trait InMemoryStore: Deref<Target = InMemoryStoreCoreFields<Self::Opportunity>> {
    type Opportunity: entities::Opportunity;

    fn new() -> Self;
}

pub struct InMemoryStoreCoreFields<T: entities::Opportunity> {
    pub opportunities: RwLock<HashMap<PermissionKey, Vec<T>>>,
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
