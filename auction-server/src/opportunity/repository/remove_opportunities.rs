use {
    super::{
        models::OpportunityRemovalReason,
        InMemoryStore,
        Repository,
    },
    crate::kernel::entities::PermissionKey,
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
        reason: OpportunityRemovalReason,
    ) -> Vec<T::Opportunity> {
        let mut write_guard = self.in_memory_store.opportunities.write().await;
        let opportunitties = write_guard.remove(&permission_key);
        drop(write_guard);

        tokio::spawn({
            let db = db.clone();
            async move {
                let now = OffsetDateTime::now_utc();
                if let Err(error) = sqlx::query("UPDATE opportunity SET removal_time = $1, removal_reason = $2 WHERE permission_key = $3 AND removal_time IS NULL")
                    .bind(PrimitiveDateTime::new(now.date(), now.time()))
                    .bind(reason)
                    .bind(permission_key.as_ref())
                    .execute(&db)
                    .await {
                        tracing::error!(error = ?error, "Failed to remove opportunities");
                    }
            }
        });

        opportunitties.unwrap_or_default()
    }
}
