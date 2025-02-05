use {
    super::{
        InMemoryStore,
        OpportunityTable,
        Repository,
    },
    crate::opportunity::entities,
};

impl<T: InMemoryStore, U: OpportunityTable<T>> Repository<T, U> {
    pub async fn get_in_memory_opportunities_by_key(
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
