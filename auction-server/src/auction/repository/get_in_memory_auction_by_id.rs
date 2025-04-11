use {
    super::Repository,
    crate::auction::{
        entities,
        service::ChainTrait,
    },
};

impl Repository {
    pub async fn get_in_memory_auction_by_id(
        &self,
        auction_id: entities::AuctionId,
    ) -> Option<entities::Auction> {
        self.in_memory_store
            .auctions
            .read()
            .await
            .get(&auction_id)
            .cloned()
    }
}
