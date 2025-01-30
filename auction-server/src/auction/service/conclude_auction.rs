use {
    super::{
        auction_manager::AuctionManager,
        update_bid_status::UpdateBidStatusInput,
        ChainTrait,
        Service,
    },
    crate::auction::entities::{
        self,
        BidStatus,
    },
    futures::future::join_all,
};

pub struct ConcludeAuctionInput<T: ChainTrait> {
    pub auction: entities::Auction<T>,
}

pub struct ConcludeAuctionWithStatusesInput<T: ChainTrait> {
    pub auction:      entities::Auction<T>,
    pub bid_statuses: Vec<(T::BidStatusType, entities::Bid<T>)>,
}

impl<T: ChainTrait> Service<T>
where
    Service<T>: AuctionManager<T>,
{
    #[tracing::instrument(skip_all, fields(auction_id, bid_ids, bid_statuses))]
    pub async fn conclude_auction_with_statuses(
        &self,
        input: ConcludeAuctionWithStatusesInput<T>,
    ) -> anyhow::Result<()> {
        tracing::Span::current().record(
            "bid_ids",
            tracing::field::display(entities::BidContainerTracing(&input.auction.bids)),
        );
        tracing::Span::current().record("auction_id", input.auction.id.to_string());
        tracing::Span::current().record("bid_statuses", format!("{:?}", input.bid_statuses));
        join_all(input.bid_statuses.into_iter().map(|(status, bid)| {
            self.update_bid_status(UpdateBidStatusInput {
                bid:        bid.clone(),
                new_status: status.clone(),
            })
        }))
        .await;

        // Refetch the auction from the in-memory store to check if all bids are finalized
        if let Some(auction) = self
            .repo
            .get_in_memory_auction_by_id(input.auction.id)
            .await
        {
            if auction.bids.iter().all(|bid| bid.status.is_finalized()) {
                self.repo
                    .conclude_auction(auction.id)
                    .await
                    .map_err(|e| anyhow::anyhow!("Failed to conclude auction: {:?}", e))?;
            }
        }

        Ok(())
    }

    /// Concludes an auction by getting the auction transaction status from the chain.
    #[tracing::instrument(skip_all)]
    pub async fn conclude_auction(&self, input: ConcludeAuctionInput<T>) -> anyhow::Result<()> {
        let auction = input.auction;
        tracing::info!(chain_id = self.config.chain_id, auction_id = ?auction.id, permission_key = auction.permission_key.to_string(), "Concluding auction");
        if let Some(tx_hash) = auction.tx_hash.clone() {
            let bids: Vec<entities::Bid<T>> = auction
                .bids
                .iter()
                .filter(|bid| !bid.status.is_finalized())
                .cloned()
                .collect();
            let bid_statuses = self
                .get_bid_results(
                    bids.clone(),
                    entities::BidStatusAuction {
                        id: auction.id,
                        tx_hash,
                    },
                )
                .await?;

            self.conclude_auction_with_statuses(ConcludeAuctionWithStatusesInput {
                auction,
                bid_statuses: bid_statuses
                    .into_iter()
                    .zip(bids)
                    .filter_map(|(status, bid)| status.map(|status| (status, bid)))
                    .collect(),
            })
            .await?;
        }
        Ok(())
    }
}
