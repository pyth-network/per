use {
    super::{
        models::{
            self,
            OpportunityMetadata,
            OpportunityRemovalReason,
        },
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
    sqlx::{
        Postgres,
        QueryBuilder,
    },
    time::{
        OffsetDateTime,
        PrimitiveDateTime,
    },
    uuid::Uuid,
};

impl<T: InMemoryStore> Repository<T> {
    /// Add an opportunity to the system, in memory and in the database.
    ///
    /// The provided opportunity will be added to the in-memory store. If the opportunity already exists
    /// in the database, then it will be refreshed. Otherwise it will be added to the database.
    pub async fn add_opportunity(
        &self,
        db: &sqlx::Pool<Postgres>,
        opportunity: <T::Opportunity as entities::Opportunity>::OpportunityCreate,
    ) -> Result<T::Opportunity, RestError> {
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

    /// Gets the opportunity from the database if it exists, otherwise adds it.
    ///
    /// If the opportunity already exists in the database and hasn't become invalid in simulation, then
    /// it will be refreshed. Otherwise, the opportunity will be added to the database.
    async fn get_or_add_db_opportunity(
        &self,
        db: &sqlx::Pool<Postgres>,
        opportunity: <T::Opportunity as entities::Opportunity>::OpportunityCreate,
    ) -> Result<T::Opportunity, RestError> {
        let chain_type = <T::Opportunity as entities::Opportunity>::ModelMetadata::get_chain_type();
        let opportunity: T::Opportunity =
            <T::Opportunity as entities::Opportunity>::new_with_current_time(opportunity);
        let metadata = opportunity.get_models_metadata();

        let result = sqlx::query!("SELECT id FROM opportunity WHERE permission_key = $1 AND chain_id = $2 AND chain_type = $3 AND sell_tokens = $4 AND buy_tokens = $5 AND metadata #- '{slot}' = $6 AND (removal_reason IS NULL OR removal_reason = $7) LIMIT 1",
        opportunity.permission_key.to_vec(),
        opportunity.chain_id,
        chain_type as _,
        serde_json::to_value(&opportunity.sell_tokens).expect("Failed to serialize sell_tokens"),
        serde_json::to_value(&opportunity.buy_tokens).expect("Failed to serialize buy_tokens"),
        self.remove_slot_field(serde_json::to_value(metadata).expect("Failed to serialize metadata")),
        OpportunityRemovalReason::Expired as _)
            .fetch_optional(db)
            .await
            .map_err(|e| {
                tracing::error!("DB: Failed to check duplicate opportunity: {}", e);
                RestError::TemporarilyUnavailable
            })?;

        match result {
            Some(record) => self.refresh_db_opportunity(db, record.id).await,
            None => self.add_db_opportunity(db, opportunity).await,
        }
    }

    fn remove_slot_field(&self, mut metadata: serde_json::Value) -> serde_json::Value {
        if let Some(obj) = metadata.as_object_mut() {
            obj.remove("slot");
        }
        metadata
    }

    /// Refresh an opportunity that already exists in the database.
    ///
    /// This will update the last creation time of the opportunity and remove the removal reason and time.
    async fn refresh_db_opportunity(
        &self,
        db: &sqlx::Pool<Postgres>,
        id: Uuid,
    ) -> Result<T::Opportunity, RestError> {
        let odt_creation = OffsetDateTime::now_utc();

        let mut query = QueryBuilder::new("UPDATE opportunity SET removal_reason = NULL, removal_time = NULL, last_creation_time = ");
        query.push_bind(PrimitiveDateTime::new(
            odt_creation.date(),
            odt_creation.time(),
        ));
        query.push(" WHERE id = ");
        query.push_bind(id);
        query.push(" RETURNING *");
        let opportunity: models::Opportunity<
            <T::Opportunity as entities::Opportunity>::ModelMetadata,
        > = query.build_query_as().fetch_one(db).await.map_err(|e| {
            tracing::error!("DB: Failed to refresh opportunity {}: {}", id, e);
            RestError::TemporarilyUnavailable
        })?;

        opportunity.clone().try_into().map_err(|_| {
            tracing::error!(
                "Failed to convert database opportunity to entity opportunity: {:?}",
                opportunity
            );
            RestError::TemporarilyUnavailable
        })
    }

    async fn add_db_opportunity(
        &self,
        db: &sqlx::Pool<Postgres>,
        opportunity: <T as InMemoryStore>::Opportunity,
    ) -> Result<T::Opportunity, RestError> {
        let chain_type = <T::Opportunity as entities::Opportunity>::ModelMetadata::get_chain_type();
        let metadata = opportunity.get_models_metadata();
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
        PrimitiveDateTime::new(opportunity.creation_time.date(), opportunity.creation_time.time()),
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

        Ok(opportunity)
    }
}
