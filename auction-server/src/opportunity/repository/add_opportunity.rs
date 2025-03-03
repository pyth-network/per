use {
    super::{
        InMemoryStore,
        Repository,
    },
    crate::{
        api::RestError,
        opportunity::entities::{
            self,
            Opportunity,
        },
    },
};

impl<T: InMemoryStore> Repository<T> {
    pub async fn add_opportunity(
        &self,
        opportunity: <T::Opportunity as entities::Opportunity>::OpportunityCreate,
    ) -> Result<T::Opportunity, RestError> {
        let opportunity: T::Opportunity = T::Opportunity::new_with_current_time(opportunity);
        self.db.add_opportunity(&opportunity).await?;
        self.in_memory_store
            .opportunities
            .write()
            .await
            .entry(opportunity.get_key())
            .or_insert_with(Vec::new)
            .push(opportunity.clone());

        Ok(opportunity)
    }
}
