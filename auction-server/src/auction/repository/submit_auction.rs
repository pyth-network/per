use {
    super::Repository,
    crate::auction::{
        entities::{
            self,
            BidStatus,
        },
        service::ChainTrait,
    },
    time::{
        OffsetDateTime,
        PrimitiveDateTime,
    },
};

impl<T: ChainTrait> Repository<T> {
    #[tracing::instrument(skip_all)]
    pub async fn submit_auction(
        &self,
        auction: entities::Auction<T>,
        transaction_hash: entities::TxHash<T>,
    ) -> anyhow::Result<entities::Auction<T>> {
        let mut auction = auction.clone();
        let now = OffsetDateTime::now_utc();
        auction.tx_hash = Some(transaction_hash.clone());
        auction.submission_time = Some(now);
        sqlx::query!("UPDATE auction SET submission_time = $1, tx_hash = $2 WHERE id = $3 AND submission_time IS NULL",
            PrimitiveDateTime::new(now.date(), now.time()),
            T::BidStatusType::convert_tx_hash(&transaction_hash),
            auction.id,
        ).execute(&self.db).await?;

        self.in_memory_store
            .submitted_auctions
            .write()
            .await
            .push(auction.clone());
        Ok(auction)
    }
}
