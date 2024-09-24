use {
    crate::{
        api::{
            bid::{
                BidResult,
                SimulatedBids,
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
        auction::{
            Bid,
            BidEvm,
            BidSvm,
        },
        config::{
            ChainId,
            RunOptions,
        },
        models,
        opportunity::api::{
            get_opportunities,
            opportunity_bid,
            post_opportunity,
            OpportunityBid,
            OpportunityMode,
            OpportunityParamsWithMetadata,
        },
        server::{
            EXIT_CHECK_INTERVAL,
            SHOULD_EXIT,
        },
        state::{
            BidStatus,
            BidStatusEvm,
            BidStatusSvm,
            BidStatusWithId,
            OpportunityParams,
            OpportunityParamsV1,
            SimulatedBid,
            SimulatedBidEvm,
            SimulatedBidSvm,
            StoreNew,
            TokenAmount,
        },
    },
    anyhow::Result,
    axum::{
        async_trait,
        extract::{
            self,
            FromRef,
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
            delete,
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
    axum_prometheus::{
        EndpointLabel,
        PrometheusMetricLayerBuilder,
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
        openapi::security::{
            Http,
            HttpAuthScheme,
            SecurityScheme,
        },
        Modify,
        OpenApi,
        ToResponse,
        ToSchema,
    },
    utoipa_redoc::{
        Redoc,
        Servable,
    },
    utoipa_swagger_ui::SwaggerUi,
};

async fn root() -> String {
    format!("Express Relay Auction Server API {}", crate_version!())
}

mod bid;
pub mod profile;
pub(crate) mod ws;

#[derive(Debug)]
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
    /// Invalid auth token
    InvalidToken,
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
            RestError::InvalidToken => (
                StatusCode::UNAUTHORIZED,
                "Invalid authorization token".to_string(),
            ),
        }
    }
}

#[derive(ToResponse, ToSchema, Serialize)]
#[response(description = "An error occurred processing the request")]
pub struct ErrorBodyResponse {
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

#[derive(Clone)]
pub enum Auth {
    Admin,
    Authorized(models::AccessTokenToken, models::Profile),
    Unauthorized,
}

#[async_trait]
impl FromRequestParts<Arc<StoreNew>> for Auth {
    type Rejection = RestError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<StoreNew>,
    ) -> Result<Self, Self::Rejection> {
        match TypedHeader::<Authorization<Bearer>>::from_request_parts(parts, state).await {
            Ok(token) => {
                let state = Arc::from_ref(state);
                let token: models::AccessTokenToken = token.token().to_string();

                let is_admin = state.store.secret_key == token;
                if is_admin {
                    return Ok(Auth::Admin);
                }

                match state.store.get_profile_by_token(&token).await {
                    Ok(profile) => Ok(Auth::Authorized(token, profile)),
                    Err(e) => Err(e),
                }
            }
            Err(e) => {
                if e.is_missing() {
                    return Ok(Auth::Unauthorized);
                }
                Err(RestError::InvalidToken)
            }
        }
    }
}

async fn admin_middleware(auth: Auth, req: extract::Request, next: middleware::Next) -> Response {
    match auth {
        Auth::Admin => next.run(req).await,
        _ => (StatusCode::FORBIDDEN, "Forbidden").into_response(),
    }
}

async fn require_login_middleware(
    auth: Auth,
    req: extract::Request,
    next: middleware::Next,
) -> Response {
    match auth {
        Auth::Authorized(_, _) => next.run(req).await,
        _ => (StatusCode::UNAUTHORIZED, "Forbidden").into_response(),
    }
}

#[macro_export]
macro_rules! admin_only {
    ($state:expr, $route:expr) => {
        $route.layer(middleware::from_fn_with_state(
            $state.clone(),
            admin_middleware,
        ))
    };
}

// Admin secret key is not considered as logged in user
#[macro_export]
macro_rules! login_required {
    ($state:expr, $route:expr) => {
        $route.layer(middleware::from_fn_with_state(
            $state.clone(),
            require_login_middleware,
        ))
    };
}

pub async fn start_api(run_options: RunOptions, store: Arc<StoreNew>) -> Result<()> {
    // Make sure functions included in the paths section have distinct names, otherwise some api generators will fail
    #[derive(OpenApi)]
    #[openapi(
    paths(
    bid::bid,
    bid::bid_status,
    bid::get_bids_by_time,
    crate::opportunity::api::post_opportunity,
    crate::opportunity::api::opportunity_bid,
    crate::opportunity::api::get_opportunities,
    profile::delete_profile_access_token,
    ),
    components(
    schemas(
    APIResponse,
    Bid,
    BidSvm,
    BidEvm,
    BidStatus,
    BidStatusEvm,
    BidStatusSvm,
    BidStatusWithId,
    BidResult,
    SimulatedBid,
    SimulatedBidEvm,
    SimulatedBidSvm,
    SimulatedBids,
    OpportunityParamsV1,
    OpportunityBid,
    OpportunityMode,
    OpportunityParams,
    OpportunityParamsWithMetadata,
    TokenAmount,
    ErrorBodyResponse,
    ClientRequest,
    ClientMessage,
    ServerResultMessage,
    ServerUpdateResponse,
    ServerResultResponse,
    ),
    responses(
    ErrorBodyResponse,
    OpportunityParamsWithMetadata,
    BidResult,
    SimulatedBids,
    ),
    ),
    tags(
    (name = "Express Relay Auction Server", description = "Auction Server handles all the necessary communications\
    between searchers and protocols. It conducts the auctions and submits the winning bids on chain.")
    ),
    modifiers(&SecurityAddon)
    )]
    struct ApiDoc;

    struct SecurityAddon;

    impl Modify for SecurityAddon {
        fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
            let components = openapi
                .components
                .as_mut()
                .expect("Should have component since it is already registered.");
            components.add_security_scheme(
                "bearerAuth",
                SecurityScheme::Http(Http::new(HttpAuthScheme::Bearer)),
            )
        }
    }

    let bid_routes = Router::new()
        .route("/", post(bid::bid))
        .route("/", login_required!(store, get(bid::get_bids_by_time)))
        .route("/:bid_id", get(bid::bid_status));
    let opportunity_routes = Router::new()
        .route("/", post(post_opportunity))
        .route("/", get(get_opportunities))
        .route("/:opportunity_id/bids", post(opportunity_bid));

    let profile_routes = Router::new()
        .route("/", admin_only!(store, post(profile::post_profile)))
        .route(
            "/access_tokens",
            admin_only!(store, post(profile::post_profile_access_token)),
        )
        .route(
            "/access_tokens",
            login_required!(store, delete(profile::delete_profile_access_token)),
        );

    let v1_routes = Router::new().nest(
        "/v1",
        Router::new()
            .nest("/bids", bid_routes)
            .nest("/opportunities", opportunity_routes)
            .nest("/profiles", profile_routes)
            .route("/ws", get(ws::ws_route_handler)),
    );

    let (prometheus_layer, _) = PrometheusMetricLayerBuilder::new()
        .with_metrics_from_fn(|| store.store.metrics_recorder.clone())
        .with_endpoint_label_type(EndpointLabel::MatchedPathWithFallbackFn(|_| {
            "unknown".to_string()
        }))
        .build_pair();
    let app: Router<()> = Router::new()
        .merge(SwaggerUi::new("/docs").url("/docs/openapi.json", ApiDoc::openapi()))
        .merge(Redoc::with_url("/redoc", ApiDoc::openapi()))
        .merge(v1_routes)
        .route("/", get(root))
        .route("/live", get(live))
        .layer(CorsLayer::permissive())
        .layer(middleware::from_extractor_with_state::<Auth, Arc<StoreNew>>(store.clone()))
        .layer(prometheus_layer)
        .with_state(store);

    let listener = tokio::net::TcpListener::bind(&run_options.server.listen_addr).await?;
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
