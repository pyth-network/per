use {
    super::Repository,
    crate::auction::{
        entities::{
            self,
            BidChainData,
            BidStatus,
        },
        service::ChainTrait,
    },
    std::collections::hash_map::Entry,
};

impl<T: ChainTrait> Repository<T> {
    async fn remove_in_memory_pending_bid(&self, bid: &entities::Bid<T>) {
        let mut write_guard = self.in_memory_store.pending_bids.write().await;
        let key = bid.chain_data.get_permission_key();
        if let Entry::Occupied(mut entry) = write_guard.entry(key.clone()) {
            let bids = entry.get_mut();
            bids.retain(|b| b.id != bid.id);
            if bids.is_empty() {
                entry.remove();
            }
        }
    }

    async fn update_in_memory_bid(&self, bid: &entities::Bid<T>, new_status: T::BidStatusType) {
        if let Some(auction_id) = new_status.get_auction_id() {
            let mut write_guard = self.in_memory_store.auctions.write().await;
            if let Some(auction) = write_guard.iter_mut().find(|a| a.id == auction_id) {
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
        let query_result = update_query.execute(&self.db).await?;

        if !new_status.is_pending() {
            self.remove_in_memory_pending_bid(&bid).await;
            self.update_in_memory_bid(&bid, new_status).await;
        }

        Ok(query_result.rows_affected() > 0)
    }
}
