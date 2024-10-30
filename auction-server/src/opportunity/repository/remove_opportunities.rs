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
    sqlx::Postgres,
    time::{
        OffsetDateTime,
        PrimitiveDateTime,
    },
};

impl<T: InMemoryStore> Repository<T> {
    pub async fn remove_opportunities(
        &self,
        db: &sqlx::Pool<Postgres>,
        permission_key: PermissionKey,
        chain_id: ChainId,
        opportunity_key: &entities::OpportunityKey,
        reason: OpportunityRemovalReason,
    ) -> anyhow::Result<Vec<T::Opportunity>> {
        let now = OffsetDateTime::now_utc();
        sqlx::query("UPDATE opportunity SET removal_time = $1, removal_reason = $2 WHERE permission_key = $3 AND chain_id = $4 and removal_time IS NULL")
            .bind(PrimitiveDateTime::new(now.date(), now.time()))
            .bind(reason)
            .bind(permission_key.as_ref())
            .bind(chain_id)
            .execute(db)
            .await?;

        let mut write_guard = self.in_memory_store.opportunities.write().await;
        let opportunitties = write_guard.remove(opportunity_key);
        drop(write_guard);

        Ok(opportunitties.unwrap_or_default())
    }
}
