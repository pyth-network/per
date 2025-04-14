use {
    super::Repository,
    crate::opportunity::{
        entities,
        entities::OpportunitySvm,
    },
};

impl Repository {
    pub async fn get_in_memory_opportunities_by_key(
        &self,
        opportunity_key: &entities::OpportunityKey,
    ) -> Vec<OpportunitySvm> {
        self.in_memory_store
            .opportunities
            .read()
            .await
            .get(opportunity_key)
            .cloned()
            .unwrap_or_default()
    }
}
