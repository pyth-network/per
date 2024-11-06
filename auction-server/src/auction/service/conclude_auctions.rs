use {
    super::{
        auctionable::Auctionable,
        ChainTrait,
        Service,
    },
    crate::auction::service::conclude_auction::ConcludeAuctionInput,
};

impl<T: ChainTrait> Service<T>
where
    Service<T>: Auctionable<T>,
{
    pub async fn conclude_auctions(&self) {
        tracing::info!(chain_id = self.config.chain_id, "Concluding auctions...");
        let auctions = self.repo.get_in_memory_submitted_auctions().await;
        for auction in auctions {
            self.task_tracker.spawn({
                let service = self.clone();
                async move {
                    let result = service
                        .conclude_auction(ConcludeAuctionInput {
                            auction: auction.clone(),
                        })
                        .await;
                    if let Err(err) = result {
                        tracing::error!(
                            error = ?err,
                            chain_id = service.config.chain_id,
                            auction_id = ?auction.id,
                            "Failed to conclude auction",
                        );
                    }
                }
            });
        }
    }
}
