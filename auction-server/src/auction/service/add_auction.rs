use {
    super::{
        auction_manager::AuctionManager,
        ChainTrait,
        Service,
    },
    crate::{
        api::RestError,
        auction::entities,
    },
};

pub struct AddAuctionInput {
    pub auction: entities::Auction,
}

impl Service
{
    pub async fn add_auction(
        &self,
        input: AddAuctionInput,
    ) -> Result<entities::Auction, RestError> {
        let auction = self.repo.add_auction(input.auction).await.map_err(|e| {
            tracing::error!(error = ?e, "Failed to add auction");
            RestError::TemporarilyUnavailable
        })?;
        self.task_tracker.spawn({
            let service = self.clone();
            async move {
                service.conclude_auction_loop(auction.id).await;
            }
        });
        Ok(auction)
    }
}
