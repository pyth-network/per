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
    async fn remove_in_memory_bid(&self, bid: &entities::Bid<T>) {
        let mut write_guard = self.in_memory_store.bids.write().await;
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
        let mut new_bid = bid.clone();
        new_bid.status = new_status.clone();

        let mut write_guard = self.in_memory_store.bids.write().await;
        let key = bid.chain_data.get_permission_key();
        match write_guard.entry(key.clone()) {
            Entry::Occupied(mut entry) => {
                let bids = entry.get_mut();
                match bids.iter().position(|b| *b == *bid) {
                    Some(index) => bids[index] = new_bid,
                    None => {
                        tracing::error!(bid = ?bid, "Update bid failed, bid not found");
                    }
                }
            }
            Entry::Vacant(_) => {
                tracing::error!(key = ?key, bid = ?bid, "Update bid failed, entry not found");
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

        if new_status.is_submitted() {
            self.update_in_memory_bid(&bid, new_status).await;
        } else if new_status.is_finalized() {
            self.remove_in_memory_bid(&bid).await;
        }

        Ok(query_result.rows_affected() > 0)
    }
}
