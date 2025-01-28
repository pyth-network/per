use {
    super::Repository,
    crate::auction::{
        entities,
        service::ChainTrait,
    },
};

impl<T: ChainTrait> Repository<T> {
    pub async fn get_in_memory_bid_by_id(
        &self,
        bid_id: entities::BidId,
    ) -> Option<entities::Bid<T>> {
        let in_memory_bids = self.get_in_memory_bids().await;
        for bids in in_memory_bids.values() {
            for bid in bids {
                if bid.id == bid_id {
                    return Some(bid.clone());
                }
            }
        }
        None
    }
}
