use {
    super::Repository,
    crate::auction::{
        entities::{
            self,
            BidStatus,
        },
        service::ChainTrait,
    },
    time::PrimitiveDateTime,
    tracing::{
        info_span,
        Instrument,
    },
};

impl<T: ChainTrait> Repository<T> {
    #[tracing::instrument(skip_all, name = "add_auction_repo", fields(auction_id))]
    pub async fn add_auction(
        &self,
        auction: entities::Auction<T>,
    ) -> anyhow::Result<entities::Auction<T>> {
        tracing::Span::current().record("auction_id", auction.id.to_string());
        sqlx::query!(
            "INSERT INTO auction (id, creation_time, permission_key, chain_id, chain_type, bid_collection_time, tx_hash) VALUES ($1, $2, $3, $4, $5, $6, $7)",
            auction.id,
            PrimitiveDateTime::new(auction.creation_time.date(), auction.creation_time.time()),
            T::convert_permission_key(&auction.permission_key),
            auction.chain_id,
            T::get_chain_type() as _,
            PrimitiveDateTime::new(auction.bid_collection_time.date(), auction.bid_collection_time.time()),
            auction.tx_hash.clone().map(|tx_hash| T::BidStatusType::convert_tx_hash(&tx_hash)),
        )
        .execute(&self.db)
            .instrument(info_span!("db_add_auction"))
        .await?;

        self.remove_in_memory_pending_bids(auction.bids.as_slice())
            .await;
        self.in_memory_store
            .auctions
            .write()
            .await
            .push(auction.clone());
        Ok(auction)
    }
}
