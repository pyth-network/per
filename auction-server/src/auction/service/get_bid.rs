use {
    super::{
        ChainTrait,
        Service,
    },
    crate::{
        api::RestError,
        auction::entities,
    },
};

pub struct GetBidInput {
    pub bid_id: entities::BidId,
}

impl<T: ChainTrait> Service<T> {
    pub async fn get_bid(&self, input: GetBidInput) -> Result<entities::Bid<T>, RestError> {
        self.repo.get_bid(input.bid_id).await
    }
}
