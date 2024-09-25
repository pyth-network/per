use {
    super::{
        models::OpportunityRemovalReason,
        InMemoryStore,
        Repository,
    },
    sqlx::Postgres,
    time::{
        OffsetDateTime,
        PrimitiveDateTime,
    },
};

impl<T: InMemoryStore> Repository<T> {
    pub async fn remove_opportunity(
        &self,
        db: &sqlx::Pool<Postgres>,
        opportunity: &T::Opportunity,
        reason: OpportunityRemovalReason,
    ) -> anyhow::Result<()> {
        let key = opportunity.permission_key.clone();
        let mut write_guard = self.in_memory_store.opportunities.write().await;
        let entry = write_guard.entry(key.clone());
        if entry
            .and_modify(|opps| opps.retain(|o| o.id != opportunity.id))
            .or_default()
            .is_empty()
        {
            write_guard.remove(&key);
        }
        drop(write_guard);

        let now = OffsetDateTime::now_utc();
        sqlx::query("UPDATE opportunity SET removal_time = $1, removal_reason = $2 WHERE id = $3 AND removal_time IS NULL")
            .bind(PrimitiveDateTime::new(now.date(), now.time()))
            .bind(reason)
            .bind(opportunity.id)
            .execute(db)
            .await?;
        Ok(())
    }
}
