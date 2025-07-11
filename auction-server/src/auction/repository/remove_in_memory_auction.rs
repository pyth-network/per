use {
    super::Repository,
    crate::auction::entities,
};

impl Repository {
    pub async fn remove_in_memory_auction(&self, auction_id: entities::AuctionId) {
        self.in_memory_store
            .auctions
            .write()
            .await
            .remove(&auction_id);
    }
}
