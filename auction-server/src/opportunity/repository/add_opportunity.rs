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
        let opportunity: T::Opportunity =
            <T::Opportunity as entities::Opportunity>::new_with_current_time(opportunity);
        let odt = OffsetDateTime::from_unix_timestamp_nanos(opportunity.creation_time * 1000)
            .expect("creation_time is valid");
        let metadata = opportunity.get_models_metadata();
        let chain_type = <T::Opportunity as entities::Opportunity>::ModelMetadata::get_chain_type();
        if self
            .check_db_duplicate_opportunity(db, &opportunity)
            .await?
        {
            tracing::warn!("Avoiding inserting duplicate opportunity to db");
        } else {
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
        }

        self.in_memory_store
            .opportunities
            .write()
            .await
            .entry(opportunity.get_key())
            .or_insert_with(Vec::new)
            .push(opportunity.clone());

        Ok(opportunity)
    }

    async fn check_db_duplicate_opportunity(
        &self,
        db: &sqlx::Pool<Postgres>,
        opportunity: &<T as InMemoryStore>::Opportunity,
    ) -> Result<bool, RestError> {
        let chain_type = <T::Opportunity as entities::Opportunity>::ModelMetadata::get_chain_type();
        let metadata = opportunity.get_models_metadata();

        let result = sqlx::query!("SELECT COUNT(*) FROM opportunity WHERE permission_key = $1 AND chain_id = $2 AND chain_type = $3 AND sell_tokens = $4 AND buy_tokens = $5 AND metadata #- '{slot}' = $6",
        opportunity.permission_key.to_vec(),
        opportunity.chain_id,
        chain_type as _,
        serde_json::to_value(&opportunity.sell_tokens).unwrap(),
        serde_json::to_value(&opportunity.buy_tokens).unwrap(),
        self.remove_slot_field(serde_json::to_value(metadata).expect("Failed to serialize metadata")))
            .fetch_one(db)
            .await
            .map_err(|e| {
                tracing::error!("DB: Failed to check duplicate opportunity: {}", e);
                RestError::TemporarilyUnavailable
            })?;

        Ok(result.count.unwrap() > 0)
    }

    fn remove_slot_field(&self, mut metadata: serde_json::Value) -> serde_json::Value {
        if let Some(obj) = metadata.as_object_mut() {
            obj.remove("slot");
        }
        metadata
    }
}
