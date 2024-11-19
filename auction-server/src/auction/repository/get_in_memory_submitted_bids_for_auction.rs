use {
    super::Repository,
    crate::auction::{
        entities::{
            self,
            BidStatus,
        },
        service::ChainTrait,
    },
};

impl<T: ChainTrait> Repository<T> {
    pub async fn get_in_memory_submitted_bids_for_auction(
        &self,
        auction: entities::Auction<T>,
    ) -> Vec<entities::Bid<T>> {
        // Filter out the bids that are in the auction and submitted
        let bids = self
            .get_in_memory_bids_by_permission_key(&auction.permission_key)
            .await;
        bids.iter()
            .filter(|bid| {
                auction
                    .bids
                    .iter()
                    .any(|auction_bid| auction_bid.id == bid.id)
                    && bid.status.is_submitted()
            })
            .cloned()
            .collect()
    }
}
