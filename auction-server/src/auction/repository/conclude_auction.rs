use {
    super::Repository,
    crate::auction::{
        entities::{
            self,
        },
        service::ChainTrait,
    },
    time::{
        OffsetDateTime,
        PrimitiveDateTime,
    },
};

impl<T: ChainTrait> Repository<T> {
    #[tracing::instrument(skip_all, name = "conclude_auction_repo", fields(auction_id))]
    pub async fn conclude_auction(&self, auction: &mut entities::Auction<T>) -> anyhow::Result<()> {
        tracing::Span::current().record("auction_id", auction.id.to_string());
        let now = OffsetDateTime::now_utc();
        auction.conclusion_time = Some(now);
        sqlx::query!(
            "UPDATE auction SET conclusion_time = $1 WHERE id = $2 AND conclusion_time IS NULL",
            PrimitiveDateTime::new(now.date(), now.time()),
            auction.id,
        )
        .execute(&self.db)
        .await?;
        Ok(())
    }
}
