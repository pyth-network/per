use {
    super::{
        InMemoryStore,
        Repository,
    },
    crate::opportunity::entities,
};

impl<T: InMemoryStore> Repository<T> {
    pub async fn get_in_memory_opportunity_by_id(
        &self,
        id: entities::OpportunityId,
    ) -> Option<T::Opportunity> {
        #[allow(clippy::mutable_key_type)]
        let opportunities = self.get_in_memory_opportunities().await;
        opportunities
            .iter()
            .find_map(|(_, values)| values.iter().find(|o| o.id == id).cloned())
    }
}
