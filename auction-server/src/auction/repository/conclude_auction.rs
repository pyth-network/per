use {
    super::Repository,
    crate::auction::entities::{
        self,
    },
};

impl Repository {
    #[tracing::instrument(skip_all, name = "conclude_auction_repo", fields(auction_id))]
    pub async fn conclude_auction(&self, auction_id: entities::AuctionId) -> anyhow::Result<()> {
        tracing::Span::current().record("auction_id", auction_id.to_string());
        self.db.conclude_auction(auction_id).await?;
        self.remove_in_memory_auction(auction_id).await;
        Ok(())
    }
}
