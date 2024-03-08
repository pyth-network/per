use {
    crate::{
        api::{
            bid::BidResult,
            liquidation::OpportunityParamsWithMetadata,
            ws::{
                APIResposne,
                ClientMessage,
                ClientRequest,
                ServerResultMessage,
                ServerResultResponse,
                ServerUpdateResponse,
            },
        },
        auction::Bid,
        config::RunOptions,
        liquidation_adapter::OpportunityBid,
        server::{
            EXIT_CHECK_INTERVAL,
            SHOULD_EXIT,
        },
        state::{
            BidStatus,
            OpportunityParams,
            OpportunityParamsV1,
            Store,
            TokenAmount,
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
    format!("Express Relay Auction Server API {}", crate_version!())
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
}

impl RestError {
    pub fn to_status_and_message(&self) -> (StatusCode, String) {
        match self {
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
        }
    }
}

#[derive(ToResponse, ToSchema, Serialize)]
#[response(description = "An error occurred processing the request")]
struct ErrorBodyResponse {
    error: String,
}

impl IntoResponse for RestError {
    fn into_response(self) -> Response {
        let (status, msg) = self.to_status_and_message();
        (status, Json(ErrorBodyResponse { error: msg })).into_response()
    }
}

pub async fn live() -> Response {
    (StatusCode::OK, "OK").into_response()
}


pub async fn start_api(run_options: RunOptions, store: Arc<Store>) -> Result<()> {
    // Make sure functions included in the paths section have distinct names, otherwise some api generators will fail
    #[derive(OpenApi)]
    #[openapi(
    paths(
    bid::bid,
    bid::bid_status,
    liquidation::post_opportunity,
    liquidation::liquidation_bid,
    liquidation::get_opportunities,
    ),
    components(
    schemas(
    APIResposne,
    Bid,
    BidStatus,
    BidResult,
    OpportunityParamsV1,
    OpportunityBid,
    OpportunityParams,
    OpportunityParamsWithMetadata,
    TokenAmount,
    BidResult,
    ErrorBodyResponse,
    ClientRequest,
    ClientMessage,
    ServerResultMessage,
    ServerUpdateResponse,
    ServerResultResponse
    ),
    responses(
    ErrorBodyResponse,
    OpportunityParamsWithMetadata,
    BidResult,
    ),
    ),
    tags(
    (name = "Express Relay Auction Server", description = "Auction Server handles all the necessary communications\
    between searchers and protocols. It conducts the auctions and submits the winning bids on chain.")
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
            post(liquidation::liquidation_bid),
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
