use {
    super::{
        InMemoryStoreEvm,
        Repository,
    },
    crate::{
        api::RestError,
        opportunity::entities,
    },
    sqlx::{
        types::BigDecimal,
        Postgres,
    },
    std::str::FromStr,
    time::{
        OffsetDateTime,
        PrimitiveDateTime,
    },
};

impl Repository<InMemoryStoreEvm> {
    pub async fn add_opportunity(
        &self,
        db: &sqlx::Pool<Postgres>,
        opportunity: entities::OpportunityEvm,
    ) -> Result<(), RestError> {
        let odt = OffsetDateTime::from_unix_timestamp_nanos(opportunity.creation_time * 1000)
            .expect("creation_time is valid");
        sqlx::query!("INSERT INTO opportunity (id,
                                                        creation_time,
                                                        permission_key,
                                                        chain_id,
                                                        target_contract,
                                                        target_call_value,
                                                        target_calldata,
                                                        sell_tokens,
                                                        buy_tokens) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        opportunity.id,
        PrimitiveDateTime::new(odt.date(), odt.time()),
        opportunity.permission_key.to_vec(),
        opportunity.chain_id,
        &opportunity.target_contract.to_fixed_bytes(),
        BigDecimal::from_str(&opportunity.target_call_value.to_string()).unwrap(),
        opportunity.target_calldata.to_vec(),
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
            .push(opportunity);
        Ok(())
    }
}
