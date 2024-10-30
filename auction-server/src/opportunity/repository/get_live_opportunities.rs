use {
    super::{
        InMemoryStore,
        Repository,
    },
    crate::opportunity::entities,
};

impl<T: InMemoryStore> Repository<T> {
    pub async fn get_live_opportunities(
        &self,
        opportunity_key: &entities::OpportunityKey,
    ) -> Vec<T::Opportunity> {
        self.in_memory_store
            .opportunities
            .read()
            .await
            .get(opportunity_key)
            .cloned()
            .unwrap_or_default()
    }
}
