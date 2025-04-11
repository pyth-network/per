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

pub struct ConcludeAuctionInput {
    pub auction: entities::Auction,
}

pub struct ConcludeAuctionWithStatusesInput {
    pub auction:      entities::Auction,
    pub bid_statuses: Vec<(entities::BidStatusSvm, entities::Bid)>,
}

impl Service
{
    #[tracing::instrument(skip_all, fields(auction_id, bid_ids, bid_statuses))]
    pub async fn conclude_auction_with_statuses(
        &self,
        input: ConcludeAuctionWithStatusesInput,
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
            if auction.bids.iter().all(|bid| bid.status.is_concluded()) {
                self.repo
                    .conclude_auction(auction.id)
                    .await
                    .map_err(|e| anyhow::anyhow!("Failed to conclude auction: {:?}", e))?;
            } else if Self::is_auction_expired(&auction) {
                // TODO This is a workaround for a bug, we need to find a better solution
                // TODO Maybe a way to make sure bids are assigned only to one auction
                // There are some cases where the bid for the auction is also in another auction
                // If the other auction is concluded, the bid status for that auction is updated
                // But the bid status for this auction is not updated and it's impossible to update it
                // If the auction is expired, we need to remove it from the in-memory store
                // To make sure that the auction is not stuck for ever
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
    async fn conclude_auction(&self, input: ConcludeAuctionInput) -> anyhow::Result<()> {
        let auction = input.auction;
        tracing::info!(chain_id = self.config.chain_id, auction_id = ?auction.id, permission_key = auction.permission_key.to_string(), "Concluding auction");
        if let Some(tx_hash) = auction.tx_hash.clone() {
            let bids: Vec<entities::Bid> = auction
                .bids
                .iter()
                .filter(|bid| !bid.status.is_concluded())
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
        } else if Self::is_auction_expired(&auction) {
            // This only happens if auction submission to chain fails
            // This is a very rare case and should not happen
            tracing::warn!("Auction has no transaction hash and is expired");
            let lost_status = entities::BidStatusSvm::new_lost();
            self.conclude_auction_with_statuses(ConcludeAuctionWithStatusesInput {
                auction:      auction.clone(),
                bid_statuses: auction
                    .bids
                    .iter()
                    .map(|bid| (lost_status.clone(), bid.clone()))
                    .collect(),
            })
            .await?;
        }
        Ok(())
    }

    pub async fn conclude_auction_loop(&self, auction_id: entities::AuctionId) {
        let mut interval = Self::get_conclusion_interval();
        loop {
            interval.tick().await;
            if let Some(auction) = self.repo.get_in_memory_auction_by_id(auction_id).await {
                if let Err(e) = self
                    .conclude_auction(ConcludeAuctionInput { auction })
                    .await
                {
                    tracing::error!(error = ?e, "Failed to conclude auction");
                }
            } else {
                break;
            }
        }
    }
}
