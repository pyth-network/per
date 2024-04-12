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
            BidId,
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
    serde::{
        Deserialize,
        Serialize,
    },
    std::sync::Arc,
    utoipa::{
        ToResponse,
        ToSchema,
    },
};

#[derive(Serialize, Deserialize, ToResponse, ToSchema, Clone)]
pub struct BidResult {
    pub status: String,
    /// The unique id created to identify the bid. This id can be used to query the status of the bid.
    #[schema(example = "beedbeed-58cc-4372-a567-0e02b2c3d479", value_type=String)]
    pub id:     BidId,
}

/// Bid on a specific permission key for a specific chain.
///
/// Your bid will be simulated and verified by the server. Depending on the outcome of the auction, a transaction
/// containing the contract call will be sent to the blockchain expecting the bid amount to be paid after the call.
#[utoipa::path(post, path = "/v1/bids", request_body = Bid, responses(
    (status = 200, description = "Bid was placed successfully", body = BidResult,
    example = json!({"status": "OK", "id": "beedbeed-b346-4fa1-8fab-2541a9e1872d"})),
    (status = 400, response = ErrorBodyResponse),
    (status = 404, description = "Chain id was not found", body = ErrorBodyResponse),
),)]
pub async fn bid(
    State(store): State<Arc<Store>>,
    Json(bid): Json<Bid>,
) -> Result<Json<BidResult>, RestError> {
    process_bid(store, bid).await
}

pub async fn process_bid(store: Arc<Store>, bid: Bid) -> Result<Json<BidResult>, RestError> {
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
    Path(bid_id): Path<BidId>,
) -> Result<Json<BidStatus>, RestError> {
    let status_json = store.get_bid_status(bid_id).await?;

    Ok(status_json)
}
