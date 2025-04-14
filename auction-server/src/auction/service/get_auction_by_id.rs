use {
    super::Service,
    crate::auction::entities,
};

pub struct GetAuctionByIdInput {
    pub auction_id: entities::AuctionId,
}

impl Service {
    pub async fn get_auction_by_id(&self, input: GetAuctionByIdInput) -> Option<entities::Auction> {
        self.repo
            .get_in_memory_auction_by_id(input.auction_id)
            .await
    }
}
