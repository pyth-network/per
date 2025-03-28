use {
    super::Repository,
    crate::auction::{
        entities,
        service::ChainTrait,
    },
};

impl<T: ChainTrait> Repository<T> {
    #[tracing::instrument(skip_all, name = "submit_auction_repo", fields(auction_id, tx_hash))]
    pub async fn submit_auction(
        &self,
        auction: entities::Auction<T>,
        transaction_hash: entities::TxHash<T>,
    ) -> anyhow::Result<entities::Auction<T>> {
        tracing::Span::current().record("auction_id", auction.id.to_string());
        tracing::Span::current().record("tx_hash", format!("{:?}", transaction_hash));

        let auction = self.db.submit_auction(&auction, &transaction_hash).await?;
        self.update_in_memory_auction(auction.clone()).await;
        Ok(auction)
    }
}
