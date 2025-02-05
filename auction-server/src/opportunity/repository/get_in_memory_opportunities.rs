use {
    super::{
        InMemoryStore,
        OpportunityTable,
        Repository,
    },
    crate::opportunity::entities,
    std::collections::HashMap,
};

impl<T: InMemoryStore, U: OpportunityTable<T>> Repository<T, U> {
    pub async fn get_in_memory_opportunities(
        &self,
    ) -> HashMap<entities::OpportunityKey, Vec<T::Opportunity>> {
        self.in_memory_store.opportunities.read().await.clone()
    }
}
