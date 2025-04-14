use {
    super::Repository,
    crate::opportunity::{
        entities,
        entities::OpportunitySvm,
    },
};

impl Repository {
    pub async fn get_in_memory_opportunity_by_id(
        &self,
        id: entities::OpportunityId,
    ) -> Option<OpportunitySvm> {
        let opportunities = self.get_in_memory_opportunities().await;
        opportunities
            .iter()
            .find_map(|(_, values)| values.iter().find(|o| o.id == id).cloned())
    }
}
