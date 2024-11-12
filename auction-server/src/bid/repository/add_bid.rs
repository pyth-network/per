use {
    super::{
        BidTrait,
        Repository,
        RepositoryTrait,
    },
    crate::{
        api::RestError,
        bid::{
            entities::{
                self,
                BidChainData,
            },
            repository::models,
            service::ServiceTrait,
        },
    },
    time::OffsetDateTime,
};

impl<T: RepositoryTrait> Repository<T> {
    pub async fn add_bid(
        &self,
        bid_create: entities::BidCreate<T>,
        chain_data: &T::ChainData,
        amount: &T::BidAmount,
    ) -> Result<entities::Bid<T>, RestError> {
        let bid_model = models::Bid::new_from(bid_create.clone(), amount, chain_data);
        let bid = bid_model.get_bid_entity(None).map_err(|e| {
            tracing::error!(error = e.to_string(), bid_create = ?bid_create, "Failed to convert bid to entity");
            RestError::TemporarilyUnavailable
        })?;

        sqlx::query!("INSERT INTO bid (id, creation_time, permission_key, chain_id, chain_type, bid_amount, status, initiation_time, profile_id, metadata) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
            bid_model.id,
            bid_model.creation_time,
            bid_model.permission_key,
            bid_model.chain_id,
            bid_model.chain_type as _,
            bid_model.bid_amount,
            bid_model.status as _,
            bid_model.initiation_time,
            bid_model.profile_id,
            serde_json::to_value(bid_model.metadata).expect("Failed to serialize metadata"),
        ).execute(&self.db)
            .await.map_err(|e| {
            tracing::error!(error = e.to_string(), bid_create = ?bid_create, "DB: Failed to insert bid");
            RestError::TemporarilyUnavailable
        })?;

        self.in_memory_store
            .bids
            .write()
            .await
            .entry(bid.chain_data.get_permission_key())
            .or_insert_with(Vec::new)
            .push(bid.clone());

        Ok(bid)
    }
}
