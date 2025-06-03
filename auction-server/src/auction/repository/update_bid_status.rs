use {
    super::Repository,
    crate::auction::entities::{
        self,
        BidStatus,
    },
    time::OffsetDateTime,
};

impl Repository {
    // Find the in memory auction which contains the bid and update the bid status
    async fn update_in_memory_auction_bid(
        &self,
        bid: &entities::Bid,
        new_status: entities::BidStatusSvm,
    ) {
        if let Some(auction_id) = new_status.get_auction_id() {
            let mut write_guard = self.in_memory_store.auctions.write().await;
            if let Some(auction) = write_guard.get_mut(&auction_id) {
                let bid_index = auction.bids.iter().position(|b| b.id == bid.id);
                if let Some(index) = bid_index {
                    auction.bids[index].status = new_status;
                }
            }
        }
    }

    /// Update the status of a bid and return true if the bid was updated
    pub async fn update_bid_status(
        &self,
        bid: entities::Bid,
        new_status: entities::BidStatusSvm,
    ) -> anyhow::Result<(bool, Option<OffsetDateTime>)> {
        let (is_updated, conclusion_time_new) =
            self.db.update_bid_status(&bid, &new_status).await?;
        if is_updated && !new_status.is_pending() {
            self.remove_in_memory_pending_bids(&[bid.clone()]).await;
            self.update_in_memory_auction_bid(&bid, new_status).await;
        }
        Ok((is_updated, conclusion_time_new))
    }
}
