use {
    super::Repository,
    crate::auction::{
        entities,
        service::ChainTrait,
    },
};

impl<T: ChainTrait> Repository<T> {
    pub async fn get_in_memory_auction_by_id(
        &self,
        auction_id: entities::AuctionId,
    ) -> Option<entities::Auction<T>> {
        self.get_in_memory_auctions()
            .await
            .into_iter()
            .find(|auction| auction.id == auction_id)
    }
}
