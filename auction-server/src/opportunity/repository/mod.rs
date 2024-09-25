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
mod get_opportunities;
mod get_opportunities_by_permission_key;
mod get_opportunities_by_permission_key_and_id;
mod get_spoof_info;
mod models;
mod opportunity_exists;
mod remove_opportunity;

#[derive(Debug)]
pub struct Repository<T: Cache> {
    pub cache: T,
}

pub trait Cache: Deref<Target = CacheCoreFields<Self::Opportunity>> {
    type Opportunity: entities::Opportunity;

    fn new() -> Self;
}

pub struct CacheCoreFields<T: entities::Opportunity> {
    pub opportunities: RwLock<HashMap<PermissionKey, Vec<T>>>,
}

impl<T: entities::Opportunity> CacheCoreFields<T> {
    pub fn new() -> Self {
        Self {
            opportunities: RwLock::new(HashMap::new()),
        }
    }
}

pub struct CacheEvm {
    pub core_fields: CacheCoreFields<entities::OpportunityEvm>,
    pub spoof_info:  RwLock<HashMap<Address, entities::SpoofState>>,
}
pub struct CacheSvm {
    pub core_fields: CacheCoreFields<entities::OpportunitySvm>,
}

impl Cache for CacheEvm {
    type Opportunity = entities::OpportunityEvm;

    fn new() -> Self {
        Self {
            core_fields: CacheCoreFields::new(),
            spoof_info:  RwLock::new(HashMap::new()),
        }
    }
}

impl Cache for CacheSvm {
    type Opportunity = entities::OpportunitySvm;

    fn new() -> Self {
        Self {
            core_fields: CacheCoreFields::new(),
        }
    }
}

impl Deref for CacheEvm {
    type Target = CacheCoreFields<entities::OpportunityEvm>;

    fn deref(&self) -> &Self::Target {
        &self.core_fields
    }
}

impl Deref for CacheSvm {
    type Target = CacheCoreFields<entities::OpportunitySvm>;

    fn deref(&self) -> &Self::Target {
        &self.core_fields
    }
}

impl<T: Cache> Repository<T> {
    pub fn new() -> Self {
        Self { cache: T::new() }
    }
}
