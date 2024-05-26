use {
    super::Auth,
    crate::{
        api::{
            ErrorBodyResponse,
            RestError,
        },
        auction::{
            handle_bid,
            Bid,
        },
        models,
        state::{
            BidId,
            BidStatus,
            SimulatedBid,
            Store,
        },
    },
    axum::{
        extract::{
            Path,
            Query,
            State,
        },
        Json,
    },
    serde::{
        Deserialize,
        Serialize,
    },
    sqlx::types::time::OffsetDateTime,
    std::sync::Arc,
    time::format_description::well_known::Rfc3339,
    utoipa::{
        IntoParams,
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
    auth: Auth,
    State(store): State<Arc<Store>>,
    Json(bid): Json<Bid>,
) -> Result<Json<BidResult>, RestError> {
    process_bid(store, bid, auth.profile.map(|p| p.id)).await
}

pub async fn process_bid(
    store: Arc<Store>,
    bid: Bid,
    profile_id: Option<models::ProfileId>,
) -> Result<Json<BidResult>, RestError> {
    match handle_bid(store, bid, OffsetDateTime::now_utc(), profile_id).await {
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

#[derive(Serialize, Deserialize, ToResponse, ToSchema, Clone)]
#[schema(title = "BidsResponse")]
pub struct SimulatedBids {
    pub items: Vec<SimulatedBid>,
}

#[derive(Serialize, Deserialize, IntoParams)]
pub struct GetBidsByTimeQueryParams {
    #[param(example="2024-05-23T21:26:57.329954Z", value_type = Option <String>)]
    pub initiation_time: Option<String>,
}

/// Returns at most 20 bids which were submitted after a specific time.
/// If no time is provided, the server will return the first bids.
#[utoipa::path(get, path = "/v1/bids",
    security(
        ("bearerAuth" = []),
    ),
    responses(
    (status = 200, description = "Paginated list of bids for the specified query", body = SimulatedBids),
    (status = 400, response = ErrorBodyResponse),
),  params(GetBidsByTimeQueryParams),
)]
pub async fn get_bids_by_time(
    auth: Auth,
    State(store): State<Arc<Store>>,
    query: Query<GetBidsByTimeQueryParams>,
) -> Result<Json<SimulatedBids>, RestError> {
    match auth.profile {
        Some(profile) => {
            let initiation_time = match query.initiation_time.clone() {
                Some(time) => {
                    Some(OffsetDateTime::parse(time.as_str(), &Rfc3339).map_err(|_| {
                        RestError::BadParameters("Invalid initiation time".to_string())
                    })?)
                }
                None => None,
            };
            let bids = store
                .get_simulated_bids_by_time(profile.id, initiation_time)
                .await?;
            Ok(Json(SimulatedBids {
                items: bids.clone(),
            }))
        }
        None => {
            tracing::error!("Unauthorized access to get_bids_by_time");
            Err(RestError::TemporarilyUnavailable)
        }
    }
}
