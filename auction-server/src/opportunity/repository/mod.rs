use {
    super::entities,
    axum_prometheus::metrics,
    ethers::types::Address,
    express_relay::state::ExpressRelayMetadata,
    solana_sdk::pubkey::Pubkey,
    std::{
        collections::HashMap,
        ops::Deref,
    },
    tokio::sync::RwLock,
};

mod add_opportunity;
mod add_spoof_info;
mod get_express_relay_metadata;
mod get_in_memory_opportunities;
mod get_in_memory_opportunities_by_key;
mod get_in_memory_opportunity_by_id;
mod get_opportunities;
mod get_spoof_info;
mod get_token_program;
mod models;
mod refresh_in_memory_opportunity;
mod remove_opportunities;
mod remove_opportunity;

pub use models::*;

pub const OPPORTUNITY_PAGE_SIZE_CAP: usize = 100;

#[derive(Debug)]
pub struct Repository<T: InMemoryStore> {
    pub in_memory_store: T,
    pub db:              Box<dyn Database<T>>,
}

pub trait InMemoryStore:
    Deref<Target = InMemoryStoreCoreFields<Self::Opportunity>> + Send + Sync + 'static
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
    pub core_fields:            InMemoryStoreCoreFields<entities::OpportunitySvm>,
    pub token_program_cache:    RwLock<HashMap<Pubkey, Pubkey>>,
    pub express_relay_metadata: RwLock<Option<ExpressRelayMetadata>>,
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
            core_fields:            InMemoryStoreCoreFields::new(),
            token_program_cache:    RwLock::new(HashMap::new()),
            express_relay_metadata: RwLock::new(None),
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
    pub fn new(db: impl Database<T>) -> Self {
        Self {
            in_memory_store: T::new(),
            db:              Box::new(db),
        }
    }
    pub(super) async fn update_metrics(&self) {
        let store = &self.in_memory_store;
        metrics::gauge!("in_memory_opportunities")
            .set(store.opportunities.read().await.len() as f64);
    }
}
