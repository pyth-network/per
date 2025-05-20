use {
    super::Repository,
    crate::opportunity::entities::{
        self,
        OpportunitySvm,
    },
    time::OffsetDateTime,
};

impl Repository {
    pub async fn remove_opportunities(
        &self,
        opportunity_key: &entities::OpportunityKey,
        reason: entities::OpportunityRemovalReason,
    ) -> anyhow::Result<(Vec<OpportunitySvm>, OffsetDateTime)> {
        let removal_time = self
            .db
            .remove_opportunities(&opportunity_key.1, &opportunity_key.0, reason.into())
            .await?;

        let mut write_guard = self.in_memory_store.opportunities.write().await;
        let opportunities = write_guard.remove(opportunity_key);
        drop(write_guard);

        Ok((opportunities.unwrap_or_default(), removal_time))
    }
}
