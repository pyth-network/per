use {
    super::{
        InMemoryStore,
        Repository,
    },
    crate::opportunity::service::ChainType,
};

impl<T: InMemoryStore> Repository<T> {
    pub async fn opportunity_exists(&self, opportunity: &T::Opportunity) -> bool {
        self.in_memory_store
            .opportunities
            .read()
            .await
            .get(&opportunity.permission_key)
            .map_or(false, |opps| opps.contains(opportunity))
    }
}
