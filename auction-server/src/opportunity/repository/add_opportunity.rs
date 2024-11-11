use {
    super::{
        models::OpportunityMetadata,
        InMemoryStore,
        Repository,
    },
    crate::{
        api::RestError,
        opportunity::entities::{
            self,
            Opportunity,
        },
    },
    sqlx::Postgres,
    time::{
        OffsetDateTime,
        PrimitiveDateTime,
    },
    uuid::Uuid,
};

impl<T: InMemoryStore> Repository<T> {
    pub async fn add_opportunity(
        &self,
        db: &sqlx::Pool<Postgres>,
        opportunity: <T::Opportunity as entities::Opportunity>::OpportunityCreate,
    ) -> Result<T::Opportunity, RestError> {
        let opportunity: T::Opportunity =
            <T::Opportunity as entities::Opportunity>::new_with_current_time(opportunity);
        let opportunity = self.get_or_add_db_opportunity(db, opportunity).await?;

        self.in_memory_store
            .opportunities
            .write()
            .await
            .entry(opportunity.get_key())
            .or_insert_with(Vec::new)
            .push(opportunity.clone());

        Ok(opportunity)
    }

    async fn get_or_add_db_opportunity(
        &self,
        db: &sqlx::Pool<Postgres>,
        opportunity: <T as InMemoryStore>::Opportunity,
    ) -> Result<T::Opportunity, RestError> {
        let chain_type = <T::Opportunity as entities::Opportunity>::ModelMetadata::get_chain_type();
        let metadata = opportunity.get_models_metadata();

        let result = sqlx::query!("SELECT id FROM opportunity WHERE permission_key = $1 AND chain_id = $2 AND chain_type = $3 AND sell_tokens = $4 AND buy_tokens = $5 AND metadata #- '{slot}' = $6 AND (removal_reason IS NULL OR removal_reason = 'expired') LIMIT 1",
        opportunity.permission_key.to_vec(),
        opportunity.chain_id,
        chain_type as _,
        serde_json::to_value(&opportunity.sell_tokens).unwrap(),
        serde_json::to_value(&opportunity.buy_tokens).unwrap(),
        self.remove_slot_field(serde_json::to_value(metadata).expect("Failed to serialize metadata")))
            .fetch_optional(db)
            .await
            .map_err(|e| {
                tracing::error!("DB: Failed to check duplicate opportunity: {}", e);
                RestError::TemporarilyUnavailable
            })?;

        match result {
            Some(record) => self.update_db_opportunity(db, opportunity, record.id).await,
            None => self.add_db_opportunity(db, opportunity).await,
        }
    }

    fn remove_slot_field(&self, mut metadata: serde_json::Value) -> serde_json::Value {
        if let Some(obj) = metadata.as_object_mut() {
            obj.remove("slot");
        }
        metadata
    }

    async fn update_db_opportunity(
        &self,
        db: &sqlx::Pool<Postgres>,
        opportunity: <T as InMemoryStore>::Opportunity,
        id: Uuid,
    ) -> Result<T::Opportunity, RestError> {
        let odt_creation =
            OffsetDateTime::from_unix_timestamp_nanos(opportunity.creation_time * 1000)
                .expect("creation_time is valid");
        sqlx::query!("UPDATE opportunity SET last_creation_time = $1, removal_reason = NULL, removal_time = NULL WHERE id = $2",
        PrimitiveDateTime::new(odt_creation.date(), odt_creation.time()),
        id)
            .execute(db)
            .await
            .map_err(|e| {
                tracing::error!("DB: Failed to update opportunity: {}", e);
                RestError::TemporarilyUnavailable
            })?;

        let mut modified_opportunity = opportunity;
        modified_opportunity.id = id;
        Ok(modified_opportunity)
    }

    async fn add_db_opportunity(
        &self,
        db: &sqlx::Pool<Postgres>,
        opportunity: <T as InMemoryStore>::Opportunity,
    ) -> Result<T::Opportunity, RestError> {
        let chain_type = <T::Opportunity as entities::Opportunity>::ModelMetadata::get_chain_type();
        let metadata = opportunity.get_models_metadata();
        let odt_creation =
            OffsetDateTime::from_unix_timestamp_nanos(opportunity.creation_time * 1000)
                .expect("creation_time is valid");
        sqlx::query!("INSERT INTO opportunity (id,
                                                        creation_time,
                                                        last_creation_time,
                                                        permission_key,
                                                        chain_id,
                                                        chain_type,
                                                        metadata,
                                                        sell_tokens,
                                                        buy_tokens) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        opportunity.id,
        PrimitiveDateTime::new(odt_creation.date(), odt_creation.time()),
        PrimitiveDateTime::new(odt_creation.date(), odt_creation.time()),
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

        Ok(opportunity)
    }
}
