use {
    super::{
        Bid,
        Repository,
    },
    crate::{
        api::RestError,
        auction::{
            entities::{
                self,
                BidChainData,
            },
            service::ChainTrait,
        },
    },
};

impl<T: ChainTrait> Repository<T> {
    pub async fn add_bid(
        &self,
        bid_create: entities::BidCreate<T>,
        chain_data: &T::BidChainDataType,
        amount: &T::BidAmountType,
    ) -> Result<entities::Bid<T>, RestError> {
        let bid_model = Bid::new(bid_create.clone(), amount, chain_data);
        let bid = bid_model.get_bid_entity(None).map_err(|e| {
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
