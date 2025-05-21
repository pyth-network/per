#[cfg(test)]
use mockall::mock;
use {
    super::entities,
    crate::kernel::analytics_db::ClickhouseInserter,
    axum_prometheus::metrics,
    express_relay::state::ExpressRelayMetadata,
    solana_sdk::pubkey::Pubkey,
    std::{
        collections::HashMap,
        ops::Deref,
    },
    tokio::sync::RwLock,
};

mod add_opportunity;
mod add_opportunity_analytics;
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

pub struct Repository {
    pub in_memory_store: InMemoryStoreSvm,
    pub db:              Box<dyn Database>,
    pub db_analytics:    Box<dyn AnalyticsDatabase>,
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

pub struct AnalyticsDatabaseInserter {
    inserter_opportunity_limo: ClickhouseInserter<OpportunityAnalyticsLimo>,
    inserter_opportunity_swap: ClickhouseInserter<OpportunityAnalyticsSwap>,
}

impl AnalyticsDatabaseInserter {
    pub fn new(client: clickhouse::Client) -> Self {
        let inserter_opportunity_limo =
            ClickhouseInserter::new(client.clone(), "opportunity_limo".to_string());
        let inserter_opportunity_swap =
            ClickhouseInserter::new(client, "opportunity_swap".to_string());
        Self {
            inserter_opportunity_limo,
            inserter_opportunity_swap,
        }
    }
}

impl Repository {
    pub fn new(db: impl Database, db_analytics: impl AnalyticsDatabase) -> Self {
        Self {
            in_memory_store: InMemoryStoreSvm::new(),
            db:              Box::new(db),
            db_analytics:    Box::new(db_analytics),
        }
    }
    pub(super) async fn update_metrics(&self) {
        let store = &self.in_memory_store;
        metrics::gauge!("in_memory_opportunities")
            .set(store.opportunities.read().await.len() as f64);
    }
}

#[cfg(test)]
mock! {
    pub AnalyticsDatabaseInserter {
        pub fn new(client: clickhouse::Client) -> Self;
    }
}
