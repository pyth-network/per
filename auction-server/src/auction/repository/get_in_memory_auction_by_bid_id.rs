use {
    super::Repository,
    crate::auction::entities,
};

impl Repository {
    pub fn get_in_memory_auction_bid_by_bid_id(
        &self,
        bid_id: entities::BidId,
    ) -> Option<entities::Bid> {
        self.get_in_memory_auctions()
            .into_iter()
            .find_map(|auction| auction.bids.iter().find(|bid| bid.id == bid_id).cloned())
    }
}
