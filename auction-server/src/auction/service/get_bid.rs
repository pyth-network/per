use {
    super::Service,
    crate::{
        api::RestError,
        auction::entities,
    },
};

pub struct GetBidInput {
    pub bid_id: entities::BidId,
}

impl Service {
    pub async fn get_bid(&self, input: GetBidInput) -> Result<entities::Bid, RestError> {
        self.repo.get_bid(input.bid_id).await
    }
}
