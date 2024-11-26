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
    #[tracing::instrument(skip_all, fields(auction_id, tx_hash, bid_ids, bid_statuses))]
    pub async fn conclude_auction(&self, input: ConcludeAuctionInput<T>) -> anyhow::Result<()> {
        let auction = input.auction;
        tracing::Span::current().record("auction_id", auction.id.to_string());
        if let Some(tx_hash) = auction.tx_hash.clone() {
            tracing::Span::current().record("tx_hash", format!("{:?}", tx_hash));
            let bids = self
                .repo
                .get_in_memory_submitted_bids_for_auction(auction.clone())
                .await;

            tracing::Span::current().record(
                "bid_ids",
                format!(
                    "{:?}",
                    bids.iter()
                        .map(|bid| bid.id.to_string())
                        .collect::<Vec<String>>()
                ),
            );

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
                tracing::Span::current().record("bid_statuses", format!("{:?}", bid_statuses));

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
