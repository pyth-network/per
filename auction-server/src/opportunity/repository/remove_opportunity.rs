use {
    super::{
        models::OpportunityRemovalReason,
        InMemoryStore,
        Repository,
    },
    crate::opportunity::entities::{
        self,
        Opportunity,
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
        reason: entities::OpportunityRemovalReason,
    ) -> anyhow::Result<()> {
        let reason: OpportunityRemovalReason = reason.into();
        let now = OffsetDateTime::now_utc();
        sqlx::query("UPDATE opportunity SET removal_time = $1, removal_reason = $2 WHERE id = $3 AND removal_time IS NULL AND last_creation_time = $4")
            .bind(PrimitiveDateTime::new(now.date(), now.time()))
            .bind(reason)
            .bind(opportunity.id)
            .bind(PrimitiveDateTime::new(
                opportunity.creation_time.date(),
                opportunity.creation_time.time(),
            ))
            .execute(db)
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
