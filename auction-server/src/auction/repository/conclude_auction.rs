use {
    super::Repository,
    crate::auction::entities::{
        self,
    },
};

impl Repository {
    pub async fn conclude_auction(&self, auction_id: entities::AuctionId) -> anyhow::Result<()> {
        self.db.conclude_auction(auction_id).await?;
        self.remove_in_memory_auction(auction_id).await;
        Ok(())
    }
}
