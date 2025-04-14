use {
    super::{
        models::OpportunityRemovalReason,
        Repository,
    },
    crate::{
        kernel::entities::{
            ChainId,
            PermissionKey,
        },
        opportunity::{
            entities,
            entities::OpportunitySvm,
        },
    },
};

impl Repository {
    pub async fn remove_opportunities(
        &self,
        permission_key: PermissionKey,
        chain_id: ChainId,
        opportunity_key: &entities::OpportunityKey,
        reason: OpportunityRemovalReason,
    ) -> anyhow::Result<Vec<OpportunitySvm>> {
        self.db
            .remove_opportunities(permission_key, chain_id, reason)
            .await?;

        let mut write_guard = self.in_memory_store.opportunities.write().await;
        let opportunities = write_guard.remove(opportunity_key);
        drop(write_guard);

        Ok(opportunities.unwrap_or_default())
    }
}
