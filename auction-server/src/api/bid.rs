use {
    crate::{
        api::{
            ErrorBodyResponse,
            RestError,
        },
        auction::{
            handle_bid,
            Bid,
        },
        state::{
            BidStatus,
            Store,
        },
    },
    axum::{
        extract::{
            Path,
            State,
        },
        Json,
    },
    ethers::signers::Signer,
    serde::{
        Deserialize,
        Serialize,
    },
    std::sync::Arc,
    utoipa::{
        ToResponse,
        ToSchema,
    },
    uuid::Uuid,
};

#[derive(Serialize, Deserialize, ToResponse, ToSchema, Clone)]
pub struct BidResult {
    pub status: String,
    /// The unique id created to identify the bid. This id can be used to query the status of the bid.
    #[schema(example = "f47ac10b-58cc-4372-a567-0e02b2c3d479", value_type=String)]
    pub id:     Uuid,
}


/// Bid on a specific permission key for a specific chain.
///
/// Your bid will be simulated and verified by the server. Depending on the outcome of the auction, a transaction
/// containing the contract call will be sent to the blockchain expecting the bid amount to be paid after the call.
#[utoipa::path(post, path = "/v1/bids", request_body = Bid, responses(
    (status = 200, description = "Bid was placed successfully", body = BidResult,
    example = json!({"status": "OK", "id": "115c5c03-b346-4fa1-8fab-2541a9e1872d"})),
    (status = 400, response = ErrorBodyResponse),
    (status = 404, description = "Chain id was not found", body = ErrorBodyResponse),
),)]
pub async fn bid(
    State(store): State<Arc<Store>>,
    Json(bid): Json<Bid>,
) -> Result<Json<BidResult>, RestError> {
    store
        .chains
        .get(&bid.chain_id)
        .ok_or(RestError::InvalidChainId)?;

    match handle_bid(store, bid).await {
        Ok(id) => Ok(BidResult {
            status: "OK".to_string(),
            id,
        }
        .into()),
        Err(e) => Err(e),
    }
}


/// Query the status of a specific bid.
#[utoipa::path(get, path = "/v1/bids/{bid_id}",
    params(("bid_id"=String, description = "Bid id to query for")),
    responses(
    (status = 200, description = "Latest status of the bid", body = BidStatus),
    (status = 400, response = ErrorBodyResponse),
    (status = 404, description = "Bid was not found", body = ErrorBodyResponse),
),)]
pub async fn bid_status(
    State(store): State<Arc<Store>>,
    Path(bid_id): Path<Uuid>,
) -> Result<Json<BidStatus>, RestError> {
    let status = store.bid_status_store.get_status(&bid_id).await;
    match status {
        Some(status) => Ok(status.into()),
        None => Err(RestError::BidNotFound),
    }
}
