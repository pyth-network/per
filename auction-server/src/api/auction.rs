use {
    crate::{
        api::RestError,
        state::{
            AuctionId,
            AuctionParams,
            AuctionParamsWithMetadata,
            Store,
        },
    },
    ethers::types::H256,
    std::sync::Arc,
};

pub async fn get_concluded_auction(
    store: Arc<Store>,
    auction_id: AuctionId,
) -> Result<AuctionParamsWithMetadata, RestError> {
    let auction = sqlx::query!("SELECT * FROM auction WHERE id = $1", auction_id)
        .fetch_one(&store.db)
        .await
        .map_err(|_| RestError::AuctionNotFound)?;

    let conclusion_time = match auction.conclusion_time {
        Some(conclusion_time) => conclusion_time.assume_utc().unix_timestamp(),
        None => return Err(RestError::AuctionNotConcluded),
    };
    let tx_hash = auction.tx_hash.as_ref();
    match tx_hash {
        Some(tx_hash) => {
            let tx_hash = H256::from_slice(tx_hash);
            Ok(AuctionParamsWithMetadata {
                id: auction.id,
                conclusion_time,
                params: AuctionParams {
                    chain_id: auction.chain_id,
                    permission_key: auction.permission_key.into(),
                    tx_hash,
                },
            })
        }
        None => Err(RestError::AuctionNotFound),
    }
}
