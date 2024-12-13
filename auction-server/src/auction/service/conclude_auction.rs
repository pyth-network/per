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
        let mut auction = input.auction;
        tracing::Span::current().record(
            "bid_ids",
            tracing::field::display(entities::BidContainerTracing(&auction.bids)),
        );
        tracing::Span::current().record("auction_id", auction.id.to_string());
        tracing::Span::current().record("bid_statuses", format!("{:?}", input.bid_statuses));
        join_all(input.bid_statuses.into_iter().map(|(status, bid)| {
            self.update_bid_status(UpdateBidStatusInput {
                bid:        bid.clone(),
                new_status: status.clone(),
            })
        }))
        .await;

        if self
            .repo
            .get_in_memory_submitted_bids_for_auction(&auction)
            .await
            .is_empty()
        {
            self.repo
                .conclude_auction(&mut auction)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to conclude auction: {:?}", e))?;
            self.repo.remove_in_memory_submitted_auction(auction).await;
        }

        Ok(())
    }

    #[tracing::instrument(skip_all, fields(auction_id, tx_hash, bid_ids))]
    pub async fn conclude_auction(&self, input: ConcludeAuctionInput<T>) -> anyhow::Result<()> {
        let auction = input.auction;
        tracing::info!(chain_id = self.config.chain_id, auction_id = ?auction.id, permission_key = auction.permission_key.to_string(), "Concluding auction");
        tracing::Span::current().record("auction_id", auction.id.to_string());
        if let Some(tx_hash) = auction.tx_hash.clone() {
            tracing::Span::current().record("tx_hash", format!("{:?}", tx_hash));
            let bids = self
                .repo
                .get_in_memory_submitted_bids_for_auction(&auction)
                .await;

            tracing::Span::current().record(
                "bid_ids",
                tracing::field::display(entities::BidContainerTracing(&bids)),
            );
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
                    .iter()
                    .zip(bids)
                    .filter_map(|(status, bid)| {
                        status.as_ref().map(|status| (status.clone(), bid.clone()))
                    })
                    .collect(),
            })
            .await?;
        }
        Ok(())
    }
}
