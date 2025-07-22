use {
    super::Repository,
    crate::auction::entities,
};

impl Repository {
    #[tracing::instrument(skip_all)]
    async fn add_in_memory_auction(&self, auction: entities::Auction) {
        self.in_memory_store.auctions.insert(auction.id, auction);
    }

    // NOTE: Do not call this function directly. Instead call `add_auction` from `Service`.
    #[tracing::instrument(skip_all, name = "add_auction_repo", fields(auction_id))]
    pub async fn add_auction(
        &self,
        auction: entities::Auction,
    ) -> anyhow::Result<entities::Auction> {
        tracing::Span::current().record("auction_id", auction.id.to_string());
        self.db.add_auction(&auction).await?;

        self.remove_in_memory_pending_bids(auction.bids.as_slice())
            .await;
        self.add_in_memory_auction(auction.clone()).await;
        Ok(auction)
    }
}
