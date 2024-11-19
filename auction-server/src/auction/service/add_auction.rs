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

pub struct AddAuctionInput<T: ChainTrait> {
    pub auction: entities::Auction<T>,
}

impl<T: ChainTrait> Service<T> {
    pub async fn add_auction(
        &self,
        input: AddAuctionInput<T>,
    ) -> Result<entities::Auction<T>, RestError> {
        self.repo.add_auction(input.auction).await.map_err(|e| {
            tracing::error!(error = ?e, "Failed to add auction");
            RestError::TemporarilyUnavailable
        })
    }
}
