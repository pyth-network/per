use {
    super::Repository,
    crate::opportunity::entities,
    time::OffsetDateTime,
};

impl Repository {
    pub async fn remove_opportunity(
        &self,
        opportunity: &entities::OpportunitySvm,
        reason: entities::OpportunityRemovalReason,
    ) -> anyhow::Result<Option<OffsetDateTime>> {
        let removal_time = self
            .db
            .remove_opportunity(opportunity, reason.into())
            .await?;

        let key = opportunity.get_key();
        let mut write_guard = self.in_memory_store.opportunities.write().await;
        let entry = write_guard.entry(key.clone());
        if entry
            .and_modify(|opps| opps.retain(|o| o != opportunity))
            .or_default()
            .is_empty()
        {
            write_guard.remove(&key);
        }
        drop(write_guard);

        Ok(removal_time)
    }
}
