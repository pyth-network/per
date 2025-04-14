use {
    super::Repository,
    crate::opportunity::{
        entities,
        entities::OpportunitySvm,
    },
    std::collections::HashMap,
};

impl Repository {
    pub async fn get_in_memory_opportunities(
        &self,
    ) -> HashMap<entities::OpportunityKey, Vec<OpportunitySvm>> {
        self.in_memory_store.opportunities.read().await.clone()
    }
}
