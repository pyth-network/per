use {
    super::Repository,
    crate::auction::entities::{
        self,
    },
    std::collections::hash_map::Entry,
};

impl Repository {
    // Remove a bid from the in memory pending bids if it exists
    #[tracing::instrument(skip_all)]
    pub async fn remove_in_memory_pending_bids(&self, bids: &[entities::Bid]) {
        let mut write_guard = self.in_memory_store.pending_bids.write().await;
        for bid in bids {
            let key = bid.chain_data.get_permission_key();
            if let Entry::Occupied(mut entry) = write_guard.entry(key.clone()) {
                let bids = entry.get_mut();
                bids.retain(|b| b.id != bid.id);
                if bids.is_empty() {
                    entry.remove();
                }
            }
        }
    }
}
