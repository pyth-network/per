use {
    super::Repository,
    crate::auction::{
        entities,
        service::ChainTrait,
    },
    time::PrimitiveDateTime,
};

impl<T: ChainTrait> Repository<T> {
    pub async fn add_auction(
        &self,
        auction: entities::Auction<T>,
    ) -> anyhow::Result<entities::Auction<T>> {
        sqlx::query!(
            "INSERT INTO auction (id, creation_time, permission_key, chain_id, chain_type, bid_collection_time) VALUES ($1, $2, $3, $4, $5, $6)",
            auction.id,
            PrimitiveDateTime::new(auction.creation_time.date(), auction.creation_time.time()),
            T::convert_permission_key(&auction.permission_key),
            auction.chain_id,
            T::get_chain_type() as _,
            PrimitiveDateTime::new(auction.bid_collection_time.date(), auction.bid_collection_time.time()),
        )
        .execute(&self.db)
        .await?;
        Ok(auction)
    }
}
