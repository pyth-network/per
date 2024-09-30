use {
    super::{
        models::OpportunityMetadata,
        InMemoryStore,
        Repository,
    },
    crate::{
        api::RestError,
        opportunity::entities,
    },
    sqlx::Postgres,
    time::{
        OffsetDateTime,
        PrimitiveDateTime,
    },
};

impl<T: InMemoryStore> Repository<T> {
    pub async fn add_opportunity(
        &self,
        db: &sqlx::Pool<Postgres>,
        opportunity: <T::Opportunity as entities::Opportunity>::OpportunityCreate,
    ) -> Result<T::Opportunity, RestError> {
        let opportunity: T::Opportunity = opportunity.into();
        let odt = OffsetDateTime::from_unix_timestamp_nanos(opportunity.creation_time * 1000)
            .expect("creation_time is valid");
        let metadata: <T::Opportunity as entities::Opportunity>::ModelMetadata =
            opportunity.clone().into();
        let chain_type = <T::Opportunity as entities::Opportunity>::ModelMetadata::get_chain_type();
        sqlx::query!("INSERT INTO opportunity (id,
                                                        creation_time,
                                                        permission_key,
                                                        chain_id,
                                                        chain_type,
                                                        metadata,
                                                        sell_tokens,
                                                        buy_tokens) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        opportunity.id,
        PrimitiveDateTime::new(odt.date(), odt.time()),
        opportunity.permission_key.to_vec(),
        opportunity.chain_id,
        chain_type as _,
        serde_json::to_value(metadata).expect("Failed to serialize metadata"),
        serde_json::to_value(&opportunity.sell_tokens).unwrap(),
        serde_json::to_value(&opportunity.buy_tokens).unwrap())
            .execute(db)
            .await
            .map_err(|e| {
                tracing::error!("DB: Failed to insert opportunity: {}", e);
                RestError::TemporarilyUnavailable
            })?;

        self.in_memory_store
            .opportunities
            .write()
            .await
            .entry(opportunity.permission_key.clone())
            .or_insert_with(Vec::new)
            .push(opportunity.clone());

        Ok(opportunity)
    }
}
