use {
    super::Repository,
    crate::auction::entities,
};

impl Repository {
    pub fn get_in_memory_auction_by_id(
        &self,
        auction_id: entities::AuctionId,
    ) -> Option<entities::Auction> {
        self.in_memory_store
            .auctions
            .get(&auction_id)
            .map(|auction_ref| auction_ref.clone())
    }
}
