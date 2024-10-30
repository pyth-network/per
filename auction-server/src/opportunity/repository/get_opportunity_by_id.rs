use {
    super::{
        InMemoryStore,
        Repository,
    },
    crate::opportunity::entities,
};

impl<T: InMemoryStore> Repository<T> {
    pub async fn get_opportunity_by_id(
        &self,
        id: entities::OpportunityId,
    ) -> Option<T::Opportunity> {
        let opportunities = self.get_all_opportunities().await;
        opportunities
            .iter()
            .find_map(|(_, values)| values.iter().find(|o| o.id == id).cloned())
    }
}
