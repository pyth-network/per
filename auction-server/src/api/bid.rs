use {
    super::Auth,
    crate::{
        api::{
            ErrorBodyResponse,
            RestError,
        },
        auction::{
            handle_bid,
            handle_bid_svm,
            Bid,
        },
        state::{
            BidId,
            BidStatus,
            SimulatedBid,
            StoreNew,
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
    utoipa::{
        IntoParams,
        ToResponse,
        ToSchema,
    },
};

#[derive(Serialize, Deserialize, ToResponse, ToSchema, Clone)]
pub struct BidResult {
    /// The status of the request. If the bid was placed successfully, the status will be "OK".
    #[schema(example = "OK")]
    pub status: String,
    /// The unique id created to identify the bid. This id can be used to query the status of the bid.
    #[schema(example = "beedbeed-58cc-4372-a567-0e02b2c3d479", value_type=String)]
    pub id:     BidId,
}

/// Bid on a specific permission key for a specific chain.
///
/// Your bid will be verified by the server. Depending on the outcome of the auction, a transaction
/// containing your bid will be sent to the blockchain expecting the bid amount to be paid in the transaction.
#[utoipa::path(post, path = "/v1/bids", request_body = Bid, responses(
    (status = 200, description = "Bid was placed successfully", body = BidResult),
    (status = 400, response = ErrorBodyResponse),
    (status = 404, description = "Chain id was not found", body = ErrorBodyResponse),
),)]
pub async fn bid(
    auth: Auth,
    State(store): State<Arc<StoreNew>>,
    Json(bid): Json<Bid>,
) -> Result<Json<BidResult>, RestError> {
    process_bid(store, bid, auth).await
}

pub async fn process_bid(
    store: Arc<StoreNew>,
    bid: Bid,
    auth: Auth,
) -> Result<Json<BidResult>, RestError> {
    let result = match bid {
        Bid::Evm(bid_evm) => {
            handle_bid(
                store.store.clone(),
                bid_evm,
                OffsetDateTime::now_utc(),
                auth,
            )
            .await
        }
        Bid::Svm(bid_svm) => {
            handle_bid_svm(store.clone(), bid_svm, OffsetDateTime::now_utc(), auth).await
        }
    };
    match result {
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
    (status = 200, body = BidStatus),
    (status = 400, response = ErrorBodyResponse),
    (status = 404, description = "Bid was not found", body = ErrorBodyResponse),
),)]
pub async fn bid_status(
    State(store): State<Arc<StoreNew>>,
    Path(bid_id): Path<BidId>,
) -> Result<Json<BidStatus>, RestError> {
    store.store.get_bid_status(bid_id).await
}

#[derive(Serialize, Deserialize, ToResponse, ToSchema, Clone)]
#[schema(title = "BidsResponse")]
pub struct SimulatedBids {
    pub items: Vec<SimulatedBid>,
}

#[derive(Serialize, Deserialize, IntoParams)]
pub struct GetBidsByTimeQueryParams {
    #[param(example="2024-05-23T21:26:57.329954Z", value_type = Option<String>)]
    #[serde(default, with = "crate::serde::nullable_datetime")]
    pub from_time: Option<OffsetDateTime>,
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
    State(store): State<Arc<StoreNew>>,
    query: Query<GetBidsByTimeQueryParams>,
) -> Result<Json<SimulatedBids>, RestError> {
    match auth {
        Auth::Authorized(_, profile) => {
            let bids = store
                .store
                .get_simulated_bids_by_time(profile.id, query.from_time)
                .await?;
            Ok(Json(SimulatedBids { items: bids }))
        }
        _ => {
            tracing::error!("Unauthorized access to get_bids_by_time");
            Err(RestError::TemporarilyUnavailable)
        }
    }
}
