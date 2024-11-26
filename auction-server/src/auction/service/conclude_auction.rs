use {
    super::{
        auction_manager::AuctionManager,
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
    Service<T>: AuctionManager<T>,
{
    pub async fn conclude_auction(&self, input: ConcludeAuctionInput<T>) -> anyhow::Result<()> {
        let mut auction = input.auction;
        if let Some(tx_hash) = auction.tx_hash.clone() {
            let bids = self
                .repo
                .get_in_memory_submitted_bids_for_auction(&auction)
                .await;

            let bid_statuses = self
                .get_bid_results(
                    bids.clone(),
                    entities::BidStatusAuction {
                        id: auction.id,
                        tx_hash,
                    },
                )
                .await?;

            join_all(
                bid_statuses
                    .iter()
                    .zip(bids.iter())
                    .filter_map(|(status, bid)| {
                        status.as_ref().map(|status| {
                            self.update_bid_status(UpdateBidStatusInput {
                                bid:        bid.clone(),
                                new_status: status.clone(),
                            })
                        })
                    }),
            )
            .await;


            if bid_statuses.iter().all(|status| status.is_some()) {
                self.repo
                    .conclude_auction(&mut auction)
                    .await
                    .map_err(|e| anyhow::anyhow!("Failed to conclude auction: {:?}", e))?;
            }

            if self
                .repo
                .get_in_memory_submitted_bids_for_auction(&auction)
                .await
                .is_empty()
            {
                self.repo.remove_in_memory_submitted_auction(auction).await;
            }
        }
        Ok(())
    }
}
