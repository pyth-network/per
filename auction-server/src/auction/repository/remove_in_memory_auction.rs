use {
    super::Repository,
    crate::auction::entities,
};

impl Repository {
    #[tracing::instrument(skip_all, fields(auction_id))]
    pub fn remove_in_memory_auction(&self, auction_id: entities::AuctionId) {
        tracing::Span::current().record("auction_id", auction_id.to_string());
        self.in_memory_store.auctions.remove(&auction_id);
    }
}
