use {
    super::{
        InMemoryStore,
        Repository,
    },
    crate::opportunity::entities::{
        self,
        OpportunityCreate,
    },
};

impl<T: InMemoryStore> Repository<T> {
    pub async fn opportunity_exists(
        &self,
        opportunity: &<T::Opportunity as entities::Opportunity>::OpportunityCreate,
    ) -> bool {
        self.in_memory_store
            .opportunities
            .read()
            .await
            .get(&opportunity.permission_key())
            .map_or(false, |opps| opps.iter().any(|opp| opp == opportunity))
    }
}
