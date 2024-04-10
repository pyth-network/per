use {
    crate::{
        api::RestError,
        state::{
            AuctionId,
            AuctionParams,
            Store,
        },
    },
    ethers::types::H256,
    serde::{
        Deserialize,
        Serialize,
    },
    std::sync::Arc,
    utoipa::ToSchema,
};

#[derive(Serialize, Deserialize, ToSchema, Clone)]
pub struct AuctionParamsWithId {
    #[schema(value_type = String)]
    pub id:     AuctionId,
    pub params: AuctionParams,
}

pub async fn get_auction_with_id(
    store: Arc<Store>,
    auction_id: AuctionId,
) -> Result<AuctionParamsWithId, RestError> {
    let auction = sqlx::query!("SELECT * FROM auction WHERE id = $1", auction_id)
        .fetch_one(&store.db)
        .await
        .map_err(|_| RestError::AuctionNotFound)?;

    Ok(AuctionParamsWithId {
        id:     auction.id,
        params: AuctionParams {
            chain_id:       auction.chain_id,
            permission_key: auction.permission_key.into(),
            tx_hash:        H256::from_slice(auction.tx_hash.as_ref()),
        },
    })
}
