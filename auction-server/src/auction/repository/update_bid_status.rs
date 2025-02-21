use {
    super::Repository,
    crate::auction::{
        entities::{
            self,
            BidStatus,
        },
        service::ChainTrait,
    },
    tracing::{
        info_span,
        Instrument,
    },
};

impl<T: ChainTrait> Repository<T> {
    // Find the in memory auction which contains the bid and update the bid status
    async fn update_in_memory_auction_bid(
        &self,
        bid: &entities::Bid<T>,
        new_status: T::BidStatusType,
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
        bid: entities::Bid<T>,
        new_status: T::BidStatusType,
    ) -> anyhow::Result<bool> {
        let update_query = T::get_update_bid_query(&bid, new_status.clone())?;
        let query_result = update_query
            .execute(&self.db)
            .instrument(info_span!("db_update_bid_status"))
            .await?;

        if query_result.rows_affected() > 0 && !new_status.is_pending() {
            self.remove_in_memory_pending_bids(&[bid.clone()]).await;
            self.update_in_memory_auction_bid(&bid, new_status).await;
        }

        Ok(query_result.rows_affected() > 0)
    }
}
