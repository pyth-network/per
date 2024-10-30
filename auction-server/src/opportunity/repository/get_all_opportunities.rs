use {
    super::{
        InMemoryStore,
        Repository,
    },
    crate::opportunity::entities,
    std::collections::HashMap,
};

impl<T: InMemoryStore> Repository<T> {
    pub async fn get_all_opportunities(
        &self,
    ) -> HashMap<entities::OpportunityKey, Vec<T::Opportunity>> {
        self.in_memory_store.opportunities.read().await.clone()
    }
}
