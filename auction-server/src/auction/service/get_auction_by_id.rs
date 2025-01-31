use {
    super::{
        ChainTrait,
        Service,
    },
    crate::auction::entities,
};

pub struct GetAuctionByIdInput {
    pub auction_id: entities::AuctionId,
}

impl<T: ChainTrait> Service<T> {
    pub async fn get_auction_by_id(
        &self,
        input: GetAuctionByIdInput,
    ) -> Option<entities::Auction<T>> {
        self.repo
            .get_in_memory_auction_by_id(input.auction_id)
            .await
    }
}
