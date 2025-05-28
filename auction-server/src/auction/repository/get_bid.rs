use {
    super::Repository,
    crate::{
        api::RestError,
        auction::entities,
    },
};

impl Repository {
    pub async fn get_bid(&self, bid_id: entities::BidId) -> Result<entities::Bid, RestError> {
        let bid = self.db.get_bid(bid_id, self.chain_id.clone()).await?;
        let auction = match bid.auction_id {
            Some(auction_id) => Some(self.db.get_auction(auction_id).await?),
            None => None,
        };

        bid.get_bid_entity(auction, None).map_err(|e| {
            tracing::error!(error = e.to_string(), bid = ?bid, "Failed to convert bid to entity");
            RestError::TemporarilyUnavailable
        })
    }
}
