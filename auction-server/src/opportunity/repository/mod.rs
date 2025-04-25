use {
    super::entities,
    axum_prometheus::metrics,
    dashmap::DashMap,
    express_relay::state::ExpressRelayMetadata,
    governor::{
        clock::DefaultClock,
        state::InMemoryState,
        RateLimiter,
    },
    solana_sdk::pubkey::Pubkey,
    std::{
        collections::HashMap,
        ops::Deref,
        sync::Arc,
    },
    tokio::sync::RwLock,
};

mod add_opportunity;
mod add_other_quotes;
mod get_express_relay_metadata;
mod get_in_memory_opportunities;
mod get_in_memory_opportunities_by_key;
mod get_in_memory_opportunity_by_id;
mod get_opportunities;
mod get_token_program;
mod models;
mod refresh_in_memory_opportunity;
mod remove_opportunities;
mod remove_opportunity;

pub use models::*;

pub const OPPORTUNITY_PAGE_SIZE_CAP: usize = 100;

type Limiter = RateLimiter<governor::state::NotKeyed, InMemoryState, DefaultClock>;

pub struct Repository {
    pub in_memory_store:    InMemoryStoreSvm,
    pub db:                 Box<dyn Database>,
    last_other_quotes_call: DashMap<String, Arc<Limiter>>,
}


pub struct InMemoryStoreCoreFields {
    pub opportunities: RwLock<HashMap<entities::OpportunityKey, Vec<entities::OpportunitySvm>>>,
}

impl InMemoryStoreCoreFields {
    pub fn new() -> Self {
        Self {
            opportunities: RwLock::new(HashMap::new()),
        }
    }
}

pub struct InMemoryStoreSvm {
    pub core_fields:            InMemoryStoreCoreFields,
    pub token_program_cache:    RwLock<HashMap<Pubkey, Pubkey>>,
    pub express_relay_metadata: RwLock<Option<ExpressRelayMetadata>>,
}

impl InMemoryStoreSvm {
    fn new() -> Self {
        Self {
            core_fields:            InMemoryStoreCoreFields::new(),
            token_program_cache:    RwLock::new(HashMap::new()),
            express_relay_metadata: RwLock::new(None),
        }
    }
}


impl Deref for InMemoryStoreSvm {
    type Target = InMemoryStoreCoreFields;

    fn deref(&self) -> &Self::Target {
        &self.core_fields
    }
}

impl Repository {
    pub fn new(db: impl Database) -> Self {
        Self {
            in_memory_store:        InMemoryStoreSvm::new(),
            db:                     Box::new(db),
            last_other_quotes_call: DashMap::new(),
        }
    }
    pub(super) async fn update_metrics(&self) {
        let store = &self.in_memory_store;
        metrics::gauge!("in_memory_opportunities")
            .set(store.opportunities.read().await.len() as f64);
    }
}
