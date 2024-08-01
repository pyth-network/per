use {
    crate::{
        api::{
            Auth,
            RestError,
        },
        auction::Bid,
        state::Store,
    },
    ethers::contract::EthEvent,
    sqlx::types::time::OffsetDateTime,
    std::{
        result,
        sync::Arc,
    },
    uuid::Uuid,
};


#[tracing::instrument(skip_all)]
pub async fn handle_bid(
    store: Arc<Store>,
    bid: Bid, // NewBid
    initiation_time: OffsetDateTime,
    auth: Auth,
) -> result::Result<Uuid, RestError> {
    Ok(Uuid::new_v4())
}
