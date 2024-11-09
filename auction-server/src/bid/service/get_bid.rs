use {
    super::{
        Service,
        ServiceTrait,
    },
    crate::{
        api::RestError,
        bid::entities,
    },
};

pub struct GetBidInput {
    pub bid_id: entities::BidId,
}

impl<T: ServiceTrait> Service<T> {
    pub async fn get_bid(&self, input: GetBidInput) -> Result<entities::Bid<T>, RestError> {
        self.repo.get_bid(input.bid_id).await
    }
}
