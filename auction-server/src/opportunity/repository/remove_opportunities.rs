use {
    super::{
        models::OpportunityRemovalReason,
        InMemoryStore,
        Repository,
    },
    crate::{
        kernel::entities::{
            ChainId,
            PermissionKey,
        },
        opportunity::entities,
    },
};

impl<T: InMemoryStore> Repository<T> {
    pub async fn remove_opportunities(
        &self,
        permission_key: PermissionKey,
        chain_id: ChainId,
        opportunity_key: &entities::OpportunityKey,
        reason: OpportunityRemovalReason,
    ) -> anyhow::Result<Vec<T::Opportunity>> {
        self.db
            .remove_opportunities(permission_key, chain_id, reason)
            .await?;

        let mut write_guard = self.in_memory_store.opportunities.write().await;
        let opportunities = write_guard.remove(opportunity_key);
        drop(write_guard);

        Ok(opportunities.unwrap_or_default())
    }
}
