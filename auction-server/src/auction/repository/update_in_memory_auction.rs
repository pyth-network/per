use {
    super::Repository,
    crate::auction::entities,
};

impl Repository {
    #[tracing::instrument(skip_all, fields(auction_id))]
    pub async fn update_in_memory_auction(&self, auction: entities::Auction) {
        tracing::Span::current().record("auction_id", auction.id.to_string());
        match self.in_memory_store.auctions.get_mut(&auction.id) {
            Some(mut a) => {
                *a = auction;
            }
            None => {
                tracing::error!(auction = ?auction, "Auction not found in in-memory store");
            }
        };
    }
}
