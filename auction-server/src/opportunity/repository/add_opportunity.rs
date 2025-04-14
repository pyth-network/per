use {
    super::Repository,
    crate::{
        api::RestError,
        opportunity::entities::{
            OpportunityCreateSvm,
            OpportunitySvm,
        },
    },
};

impl Repository {
    pub async fn add_opportunity(
        &self,
        opportunity: OpportunityCreateSvm,
    ) -> Result<OpportunitySvm, RestError> {
        let opportunity = OpportunitySvm::new_with_current_time(opportunity);
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
