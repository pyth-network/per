use {
    crate::{
        api::{
            bid::{
                Bid,
                BidResult,
            },
            liquidation::{
                OpportunityBid,
                OpportunityParamsWithMetadata,
            },
            ws::{
                ClientMessage,
                ClientRequest,
                ServerResultMessage,
                ServerResultResponse,
                ServerUpdateResponse,
            },
        },
        config::RunOptions,
        server::{
            EXIT_CHECK_INTERVAL,
            SHOULD_EXIT,
        },
        state::{
            OpportunityParams,
            OpportunityParamsV1,
            Store,
            TokenQty,
        },
    },
    anyhow::Result,
    axum::{
        http::StatusCode,
        response::{
            IntoResponse,
            Response,
        },
        routing::{
            get,
            post,
        },
        Json,
        Router,
    },
    clap::crate_version,
    ethers::types::Bytes,
    serde::Serialize,
    std::sync::{
        atomic::Ordering,
        Arc,
    },
    tower_http::cors::CorsLayer,
    utoipa::{
        OpenApi,
        ToResponse,
        ToSchema,
    },
    utoipa_swagger_ui::SwaggerUi,
};

async fn root() -> String {
    format!("PER Auction Server API {}", crate_version!())
}

mod bid;
pub(crate) mod liquidation;
pub(crate) mod ws;

pub enum RestError {
    /// The request contained invalid parameters
    BadParameters(String),
    /// The submitted opportunity was not valid
    InvalidOpportunity(String),
    /// The chain id is not supported
    InvalidChainId,
    /// The simulation failed
    SimulationError { result: Bytes, reason: String },
    /// The order was not found
    OpportunityNotFound,
    /// The bid was not found
    BidNotFound,
    /// Internal error occurred during processing the request
    TemporarilyUnavailable,
    /// A catch-all error for all other types of errors that could occur during processing.
    Unknown,
}

#[derive(ToResponse, ToSchema, Serialize)]
#[response(description = "An error occurred processing the request")]
struct ErrorBodyResponse {
    error: String,
}

impl IntoResponse for RestError {
    fn into_response(self) -> Response {
        let (status, msg) = match self {
            RestError::BadParameters(msg) => {
                (StatusCode::BAD_REQUEST, format!("Bad parameters: {}", msg))
            }
            RestError::InvalidOpportunity(msg) => (
                StatusCode::BAD_REQUEST,
                format!("Invalid opportunity: {}", msg),
            ),
            RestError::InvalidChainId => (
                StatusCode::NOT_FOUND,
                "The chain id is not found".to_string(),
            ),
            RestError::SimulationError { result, reason } => (
                StatusCode::BAD_REQUEST,
                format!("Simulation failed: {} ({})", result, reason),
            ),
            RestError::OpportunityNotFound => (
                StatusCode::NOT_FOUND,
                "Opportunity with the specified id was not found".to_string(),
            ),
            RestError::BidNotFound => (
                StatusCode::NOT_FOUND,
                "Bid with the specified id was not found".to_string(),
            ),
            RestError::TemporarilyUnavailable => (
                StatusCode::SERVICE_UNAVAILABLE,
                "This service is temporarily unavailable".to_string(),
            ),
            RestError::Unknown => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "An unknown error occurred processing the request".to_string(),
            ),
        };
        (status, Json(ErrorBodyResponse { error: msg })).into_response()
    }
}

pub async fn live() -> Response {
    (StatusCode::OK, "OK").into_response()
}


pub async fn start_api(run_options: RunOptions, store: Arc<Store>) -> Result<()> {
    #[derive(OpenApi)]
    #[openapi(
    paths(
    bid::bid,
    liquidation::post_opportunity,
    liquidation::post_bid,
    liquidation::get_opportunities,
    ),
    components(
    schemas(Bid),
    schemas(OpportunityParamsV1),
    schemas(OpportunityBid),
    schemas(OpportunityParams),
    schemas(OpportunityParamsWithMetadata),
    schemas(TokenQty),
    schemas(BidResult),
    schemas(ErrorBodyResponse),
    schemas(ClientRequest),
    schemas(ClientMessage),
    schemas(ServerResultMessage),
    schemas(ServerUpdateResponse),
    schemas(ServerResultResponse),
    responses(ErrorBodyResponse),
    responses(OpportunityParamsWithMetadata),
    responses(BidResult)
    ),
    tags(
    (name = "PER Auction", description = "Pyth Express Relay Auction Server")
    )
    )]
    struct ApiDoc;

    let app: Router<()> = Router::new()
        .merge(SwaggerUi::new("/docs").url("/docs/openapi.json", ApiDoc::openapi()))
        .route("/", get(root))
        .route("/v1/bids", post(bid::bid))
        .route("/v1/bids/:bid_id", get(bid::bid_status))
        .route(
            "/v1/liquidation/opportunities",
            post(liquidation::post_opportunity),
        )
        .route(
            "/v1/liquidation/opportunities",
            get(liquidation::get_opportunities),
        )
        .route(
            "/v1/liquidation/opportunities/:opportunity_id/bids",
            post(liquidation::post_bid),
        )
        .route("/v1/ws", get(ws::ws_route_handler))
        .route("/live", get(live))
        .layer(CorsLayer::permissive())
        .with_state(store);

    axum::Server::bind(&run_options.server.listen_addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(async {
            while !SHOULD_EXIT.load(Ordering::Acquire) {
                tokio::time::sleep(EXIT_CHECK_INTERVAL).await;
            }
            tracing::info!("Shutting down RPC server...");
        })
        .await?;
    Ok(())
}
