use {
    super::{
        db::OpportunityTable,
        InMemoryStore,
        Repository,
    },
    crate::opportunity::entities::{
        self,
        Opportunity,
    },
};

impl<T: InMemoryStore> Repository<T> {
    pub async fn remove_opportunity(
        &self,
        opportunity: &T::Opportunity,
        reason: entities::OpportunityRemovalReason,
    ) -> anyhow::Result<()> {
        self.db
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

        Ok(())
    }
}
