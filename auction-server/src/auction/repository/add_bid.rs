use {
    super::{
        Bid,
        Repository,
    },
    crate::{
        api::RestError,
        auction::entities::{
            self,
        },
    },
};

impl Repository {
    #[tracing::instrument(skip_all, err(level = tracing::Level::TRACE))]
    pub async fn add_bid(
        &self,
        bid_create: entities::BidCreate,
        chain_data: &entities::BidChainDataSvm,
        amount: &entities::BidAmountSvm,
    ) -> Result<entities::Bid, RestError> {
        let bid_model = Bid::new(bid_create.clone(), amount, chain_data);
        let bid = bid_model.get_bid_entity(None, bid_create.chain_data.get_opportunity_id()).map_err(|e| {
            tracing::error!(error = e.to_string(), bid_create = ?bid_create, "Failed to convert bid to entity");
            RestError::TemporarilyUnavailable
        })?;
        self.db.add_bid(&bid_model).await?;

        self.in_memory_store
            .pending_bids
            .write()
            .await
            .entry(bid.chain_data.get_permission_key())
            .or_insert_with(Vec::new)
            .push(bid.clone());

        Ok(bid)
    }
}
