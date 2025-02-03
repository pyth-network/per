use {
    super::Repository,
    crate::auction::{
        entities,
        service::ChainTrait,
    },
};

impl<T: ChainTrait> Repository<T> {
    pub async fn get_in_memory_auction_bid_by_id(
        &self,
        bid_id: entities::BidId,
    ) -> Option<(entities::Bid<T>, entities::Auction<T>)> {
        self.get_in_memory_auctions()
            .await
            .into_iter()
            .find_map(|auction| {
                auction
                    .bids
                    .iter()
                    .find(|bid| bid.id == bid_id)
                    .map(|bid| (bid.clone(), auction.clone()))
            })
    }
}
