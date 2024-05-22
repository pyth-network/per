use {
    crate::{
        api::{
            bid::BidResult,
            opportunity::{
                EIP712Domain,
                OpportunityParamsWithMetadata,
            },
            profile::{
                AccessToken,
                CreateAccessToken,
                CreateProfile,
                Profile,
            },
            ws::{
                APIResponse,
                ClientMessage,
                ClientRequest,
                ServerResultMessage,
                ServerResultResponse,
                ServerUpdateResponse,
            },
        },
        auction::Bid,
        config::{
            ChainId,
            RunOptions,
        },
        opportunity_adapter::OpportunityBid,
        server::{
            EXIT_CHECK_INTERVAL,
            SHOULD_EXIT,
        },
        state::{
            BidStatus,
            BidStatusWithId,
            OpportunityParams,
            OpportunityParamsV1,
            Store,
            TokenAmount,
        },
    },
    anyhow::Result,
    axum::{
        async_trait,
        extract::{
            self,
            FromRequestParts,
        },
        http::{
            request::Parts,
            StatusCode,
        },
        middleware,
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
    axum_extra::{
        headers::{
            authorization::Bearer,
            Authorization,
        },
        TypedHeader,
    },
    clap::crate_version,
    ethers::types::Bytes,
    serde::{
        Deserialize,
        Serialize,
    },
    std::sync::{
        atomic::Ordering,
        Arc,
    },
    tower_http::cors::CorsLayer,
    utoipa::{
        IntoParams,
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
pub(crate) mod opportunity;
pub mod profile;
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
    /// The opportunity was not found
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

#[derive(Serialize, Deserialize, IntoParams)]
pub struct ChainIdQueryParams {
    #[param(example = "op_sepolia", value_type = Option < String >)]
    pub chain_id: Option<ChainId>,
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

pub struct Auth {
    profile:  Option<Profile>,
    is_admin: bool,
}

#[async_trait]
impl<S> FromRequestParts<S> for Auth
where
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match TypedHeader::<Authorization<Bearer>>::from_request_parts(parts, state).await {
            Ok(_token) => Ok(Self {
                profile:  None,
                is_admin: true,
            }),
            Err(_) => Ok(Self {
                profile:  None,
                is_admin: false,
            }),
        }
    }
}

async fn auth(auth: Auth, req: extract::Request, next: middleware::Next) -> Response {
    println!("auth: {:?}", auth.is_admin);
    next.run(req).await
}

#[macro_export]
macro_rules! admin_only {
    ($route:expr) => {
        $route.layer(middleware::from_fn(auth))
    };
}

pub async fn start_api(run_options: RunOptions, store: Arc<Store>) -> Result<()> {
    // Make sure functions included in the paths section have distinct names, otherwise some api generators will fail
    #[derive(OpenApi)]
    #[openapi(
    paths(
    bid::bid,
    bid::bid_status,
    opportunity::post_opportunity,
    opportunity::opportunity_bid,
    opportunity::get_opportunities,
    profile::post_profile,
    profile::post_profile_access_token,
    ),
    components(
    schemas(
    APIResponse,
    Bid,
    BidStatus,
    BidStatusWithId,
    BidResult,
    EIP712Domain,
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
    ServerResultResponse,
    Profile,
    CreateProfile,
    CreateAccessToken,
    AccessToken,
    ),
    responses(
    ErrorBodyResponse,
    OpportunityParamsWithMetadata,
    BidResult,
    Profile,
    ),
    ),
    tags(
    (name = "Express Relay Auction Server", description = "Auction Server handles all the necessary communications\
    between searchers and protocols. It conducts the auctions and submits the winning bids on chain.")
    )
    )]
    struct ApiDoc;

    let bid_routes = Router::new()
        .route("/", post(bid::bid))
        .route("/:bid_id", get(bid::bid_status));
    let opportunity_routes = Router::new()
        .route("/", post(opportunity::post_opportunity))
        .route("/", get(opportunity::get_opportunities))
        .route("/:opportunity_id/bids", post(opportunity::opportunity_bid));
    let profile_routes = Router::new()
        .route("/", admin_only!(post(profile::post_profile)))
        .route(
            "/access_tokens",
            post(admin_only!(post(profile::post_profile_access_token))),
        );


    let v1_routes = Router::new().nest(
        "/v1",
        Router::new()
            .nest("/bids", bid_routes)
            .nest("/opportunities", opportunity_routes)
            .nest("/profiles", profile_routes)
            .route("/ws", get(ws::ws_route_handler)),
    );

    let app: Router<()> = Router::new()
        .merge(SwaggerUi::new("/docs").url("/docs/openapi.json", ApiDoc::openapi()))
        .merge(v1_routes)
        .route("/", get(root))
        .route("/live", get(live))
        .layer(CorsLayer::permissive())
        .layer(middleware::from_extractor::<Auth>())
        .with_state(store);

    let listener = tokio::net::TcpListener::bind(&run_options.server.listen_addr)
        .await
        .unwrap();
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            while !SHOULD_EXIT.load(Ordering::Acquire) {
                tokio::time::sleep(EXIT_CHECK_INTERVAL).await;
            }
            tracing::info!("Shutting down RPC server...");
        })
        .await?;
    Ok(())
}
