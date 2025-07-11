use {
    super::Repository,
    crate::auction::entities,
};

impl Repository {
    async fn add_in_memory_auction(&self, auction: entities::Auction) {
        self.in_memory_store
            .auctions
            .write()
            .await
            .insert(auction.id, auction);
    }

    // NOTE: Do not call this function directly. Instead call `add_auction` from `Service`.
    pub async fn add_auction(
        &self,
        auction: entities::Auction,
    ) -> anyhow::Result<entities::Auction> {
        self.db.add_auction(&auction).await?;

        self.remove_in_memory_pending_bids(auction.bids.as_slice())
            .await;
        self.add_in_memory_auction(auction.clone()).await;
        Ok(auction)
    }
}
