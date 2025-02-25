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

pub struct AddAuctionInput<T: ChainTrait> {
    pub auction: entities::Auction<T>,
}

impl<T: ChainTrait> Service<T>
where
    Service<T>: AuctionManager<T>,
{
    pub async fn add_auction(
        &self,
        input: AddAuctionInput<T>,
    ) -> Result<entities::Auction<T>, RestError> {
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
