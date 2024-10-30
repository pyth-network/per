use {
    super::{
        ChainType,
        Service,
    },
    crate::opportunity::{
        entities,
        repository::InMemoryStore,
    },
};

pub struct GetOpportunitiesInput {
    pub key: entities::OpportunityKey,
}

impl<T: ChainType> Service<T> {
    pub async fn get_live_opportunities(
        &self,
        input: GetOpportunitiesInput,
    ) -> Vec<<T::InMemoryStore as InMemoryStore>::Opportunity> {
        self.repo
            .get_in_memory_opportunities_by_key(&input.key)
            .await
    }
}
