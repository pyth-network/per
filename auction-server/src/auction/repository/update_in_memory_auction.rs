use {
    super::Repository,
    crate::auction::entities,
};

impl Repository {
    pub async fn update_in_memory_auction(&self, auction: entities::Auction) {
        let mut write_guard = self.in_memory_store.auctions.write().await;
        match write_guard.get_mut(&auction.id) {
            Some(a) => {
                *a = auction;
            }
            None => {
                tracing::error!(auction = ?auction, "Auction not found in in-memory store");
            }
        };
    }
}
