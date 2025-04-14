use {
    super::Service,
    crate::opportunity::{
        entities,
        entities::OpportunitySvm,
    },
};

#[derive(Debug)]
pub struct GetLiveOpportunitiesInput {
    pub key: entities::OpportunityKey,
}

impl Service {
    pub async fn get_live_opportunities(
        &self,
        input: GetLiveOpportunitiesInput,
    ) -> Vec<OpportunitySvm> {
        self.repo
            .get_in_memory_opportunities_by_key(&input.key)
            .await
    }
}
