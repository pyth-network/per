use {
    super::{
        models::OpportunityMetadata,
        InMemoryStore,
        Repository,
    },
    crate::{
        api::RestError,
        opportunity::{
            entities,
            entities::Opportunity,
        },
    },
    sqlx::Postgres,
    time::PrimitiveDateTime,
};

impl<T: InMemoryStore> Repository<T> {
    pub async fn add_opportunity(
        &self,
        db: &sqlx::Pool<Postgres>,
        opportunity: <T::Opportunity as entities::Opportunity>::OpportunityCreateAssociatedType,
    ) -> Result<T::Opportunity, RestError> {
        let opportunity: T::Opportunity =
            <T::Opportunity as entities::Opportunity>::new_with_current_time(opportunity);
        let metadata = opportunity.get_models_metadata();
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
        PrimitiveDateTime::new(opportunity.creation_time.date(), opportunity.creation_time.time()),
        opportunity.permission_key.to_vec(),
        opportunity.chain_id,
        chain_type as _,
        serde_json::to_value(metadata).expect("Failed to serialize metadata"),
        serde_json::to_value(&opportunity.sell_tokens).expect("Failed to serialize sell_tokens"),
        serde_json::to_value(&opportunity.buy_tokens).expect("Failed to serialize buy_tokens"))
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
            .entry(opportunity.get_key())
            .or_insert_with(Vec::new)
            .push(opportunity.clone());

        Ok(opportunity)
    }
}
