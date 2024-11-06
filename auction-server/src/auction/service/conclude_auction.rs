use {
    super::{
        auctionable::Auctionable,
        update_bid_status::UpdateBidStatusInput,
        ChainTrait,
        Service,
    },
    crate::auction::entities::{
        self,
    },
    futures::future::join_all,
};

pub struct ConcludeAuctionInput<T: ChainTrait> {
    pub auction: entities::Auction<T>,
}

impl<T: ChainTrait> Service<T>
where
    Service<T>: Auctionable<T>,
{
    pub async fn conclude_auction(&self, input: ConcludeAuctionInput<T>) -> anyhow::Result<()> {
        let auction = input.auction;
        if let Some(tx_hash) = auction.tx_hash.clone() {
            let bids = self
                .repo
                .get_in_memory_submitted_bids_for_auction(auction.clone())
                .await;

            if let Some(bid_statuses) = self
                .get_bid_results(
                    bids.clone(),
                    entities::BidStatusAuction {
                        id: auction.id,
                        tx_hash,
                    },
                )
                .await?
            {
                let auction = self
                    .repo
                    .conclude_auction(auction)
                    .await
                    .map_err(|e| anyhow::anyhow!("Failed to conclude auction: {:?}", e))?;

                join_all(
                    bid_statuses
                        .iter()
                        .enumerate()
                        .filter_map(|(index, bid_status)| match bids.get(index) {
                            Some(bid) => Some(self.update_bid_status(UpdateBidStatusInput {
                                bid:        bid.clone(),
                                new_status: bid_status.clone(),
                            })),
                            None => {
                                tracing::error!(
                                    bids = ?bids,
                                    bid_statuses = ?bid_statuses,
                                    auction = ?auction,
                                    "Bids array is smaller than statuses array",
                                );
                                None
                            }
                        }),
                )
                .await;

                if self
                    .repo
                    .get_in_memory_submitted_bids_for_auction(auction.clone())
                    .await
                    .is_empty()
                {
                    self.repo.remove_in_memory_submitted_auction(auction).await;
                }
            }
        }
        Ok(())
    }
}
