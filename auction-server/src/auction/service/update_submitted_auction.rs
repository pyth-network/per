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

pub struct UpdateSubmittedAuctionInput<T: ChainTrait> {
    pub auction:          entities::Auction<T>,
    pub transaction_hash: entities::TxHash<T>,
}

impl<T: ChainTrait> Service<T> {
    pub async fn update_submitted_auction(
        &self,
        input: UpdateSubmittedAuctionInput<T>,
    ) -> Result<entities::Auction<T>, RestError> {
        self.repo
            .submit_auction(input.auction, input.transaction_hash)
            .await
            .map_err(|e| {
                tracing::error!(error = ?e, "Failed to update submitted auction");
                RestError::TemporarilyUnavailable
            })
    }
}
