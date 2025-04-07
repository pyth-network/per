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

        let updated_auction = self.db.submit_auction(&auction, &transaction_hash).await?;
        if let Some(updated_auction) = updated_auction {
            self.update_in_memory_auction(updated_auction.clone()).await;
            Ok(updated_auction)
        } else {
            Ok(auction)
        }
    }
}
