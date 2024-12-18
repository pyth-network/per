use {
    crate::{
        auction::api as bid,
        config::RunOptions,
        models,
        opportunity::api as opportunity,
        server::{
            EXIT_CHECK_INTERVAL,
            SHOULD_EXIT,
        },
        state::StoreNew,
    },
    anyhow::Result,
    axum::{
        async_trait,
        extract::{
            self,
            FromRef,
            FromRequestParts,
        },
        handler::Handler,
        http::{
            request::Parts,
            Method,
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
            patch,
            post,
            put,
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
    express_relay_api_types::{
        self as api_types,
        profile::Route as ProfileRoute,
        AccessLevel,
        ErrorBodyResponse,
        Routable,
        Route,
    },
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
    },
    utoipa_redoc::{
        Redoc,
        Servable,
    },
};

async fn root() -> String {
    format!("Express Relay Auction Server API {}", crate_version!())
}

pub mod profile;
pub(crate) mod ws;

#[derive(Debug, Clone)]
pub enum RestError {
    /// The request contained invalid parameters.
    BadParameters(String),
    /// The submitted opportunity was not valid.
    InvalidOpportunity(String),
    /// The chain id is not supported.
    InvalidChainId,
    /// The simulation failed.
    SimulationError { result: Bytes, reason: String },
    /// The opportunity was not found.
    OpportunityNotFound,
    /// The bid was not found.
    BidNotFound,
    /// Internal error occurred during processing the request.
    TemporarilyUnavailable,
    /// Auth token is invalid.
    InvalidToken,
    /// The request is forbidden.
    Forbidden,
    /// The request is unauthorized.
    Unauthorized,
    /// The profile was not found.
    ProfileNotFound,
    /// The quote was not found.
    QuoteNotFound,
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
            RestError::ProfileNotFound => (
                StatusCode::NOT_FOUND,
                "Profile with the specified email was not found".to_string(),
            ),
            RestError::Forbidden => (StatusCode::FORBIDDEN, "Forbidden".to_string()),
            RestError::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized".to_string()),
            RestError::QuoteNotFound => (
                StatusCode::NOT_FOUND,
                "No quote is currently available".to_string(),
            ),
        }
    }
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

#[derive(Clone, Debug)]
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

pub async fn require_login_middleware(
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

fn remove_discriminators(doc: &mut serde_json::Value) {
    // Recursively remove all "discriminator" fields from the OpenAPI document
    if let Some(obj) = doc.as_object_mut() {
        obj.retain(|key, _| key != "discriminator");
        for value in obj.values_mut() {
            remove_discriminators(value);
        }
    }
}

pub struct WrappedRouter {
    store:      Arc<StoreNew>,
    pub router: Router<Arc<StoreNew>>,
}

impl WrappedRouter {
    pub fn new(store: Arc<StoreNew>) -> Self {
        Self {
            store,
            router: Router::new(),
        }
    }

    pub fn route<H, T>(self, path: impl Routable, handler: H) -> Self
    where
        H: Handler<T, Arc<StoreNew>>,
        T: 'static,
    {
        let properties = path.properties();
        let router = match properties.method {
            Method::GET => get(handler),
            Method::POST => post(handler),
            Method::DELETE => delete(handler),
            Method::PUT => put(handler),
            Method::PATCH => patch(handler),
            _ => panic!("Unsupported method"),
        };

        let router = match properties.access_level {
            AccessLevel::Admin => admin_only!(self.store, router),
            AccessLevel::LoggedIn => login_required!(self.store, router),
            AccessLevel::Public => router,
        };

        Self {
            store:  self.store,
            router: self.router.route(&properties.full_path, router),
        }
    }
}

pub async fn start_api(run_options: RunOptions, store: Arc<StoreNew>) -> Result<()> {
    // Make sure functions included in the paths section have distinct names, otherwise some api generators will fail
    #[derive(OpenApi)]
    #[openapi(
    paths(
    bid::post_bid,
    bid::get_bid_status,
    bid::get_bids_by_time,
    bid::get_bids_by_time_deprecated,
    bid::get_bid_status_deprecated,

    opportunity::post_opportunity,
    opportunity::opportunity_bid,
    opportunity::get_opportunities,
    opportunity::post_quote,
    opportunity::delete_opportunities,

    profile::delete_profile_access_token,
    ),
    components(
    schemas(
    api_types::ws::APIResponse,
    api_types::bid::BidCreate,
    api_types::bid::BidCreateEvm,
    api_types::bid::BidCreateSvm,
    api_types::bid::BidStatus,
    api_types::bid::BidStatusEvm,
    api_types::bid::BidStatusSvm,
    api_types::bid::BidStatusWithId,
    api_types::bid::BidResult,
    api_types::bid::Bid,
    api_types::bid::BidEvm,
    api_types::bid::BidSvm,
    api_types::bid::Bids,
    api_types::SvmChainUpdate,

    api_types::opportunity::OpportunityBidEvm,
    api_types::opportunity::OpportunityBidResult,
    api_types::opportunity::OpportunityMode,
    api_types::opportunity::OpportunityCreate,
    api_types::opportunity::OpportunityCreateEvm,
    api_types::opportunity::OpportunityCreateSvm,
    api_types::opportunity::OpportunityCreateV1Evm,
    api_types::opportunity::OpportunityCreateV1Svm,
    api_types::opportunity::OpportunityCreateProgramParamsV1Svm,
    api_types::opportunity::Opportunity,
    api_types::opportunity::OpportunityEvm,
    api_types::opportunity::OpportunitySvm,
    api_types::opportunity::TokenAmountEvm,
    api_types::opportunity::TokenAmountSvm,
    api_types::opportunity::OpportunityParamsSvm,
    api_types::opportunity::OpportunityParamsEvm,
    api_types::opportunity::OpportunityParamsV1Svm,
    api_types::opportunity::OpportunityParamsV1Evm,
    api_types::opportunity::QuoteCreate,
    api_types::opportunity::QuoteCreateSvm,
    api_types::opportunity::QuoteCreateV1Svm,
    api_types::opportunity::QuoteCreatePhantomV1Svm,
    api_types::opportunity::Quote,
    api_types::opportunity::QuoteSvm,
    api_types::opportunity::QuoteV1Svm,
    api_types::opportunity::OpportunityDelete,
    api_types::opportunity::OpportunityDeleteSvm,
    api_types::opportunity::OpportunityDeleteEvm,
    api_types::opportunity::OpportunityDeleteV1Svm,
    api_types::opportunity::OpportunityDeleteV1Evm,
    api_types::opportunity::ProgramSvm,

    ErrorBodyResponse,
    api_types::ws::ClientRequest,
    api_types::ws::ClientMessage,
    api_types::ws::ServerResultMessage,
    api_types::ws::ServerUpdateResponse,
    api_types::ws::ServerResultResponse,
    ),
    responses(
    ErrorBodyResponse,
    api_types::opportunity::Opportunity,
    api_types::bid::BidResult,
    api_types::bid::Bids,
    ),
    ),
    tags(
    (name = "Express Relay Auction Server", description = "Auction Server handles all the necessary communications \
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

    let profile_routes = WrappedRouter::new(store.clone())
        .route(ProfileRoute::PostProfile, profile::post_profile)
        .route(ProfileRoute::GetProfile, profile::get_profile)
        .route(
            ProfileRoute::PostProfileAccessToken,
            profile::post_profile_access_token,
        )
        .route(
            ProfileRoute::DeleteProfileAccessToken,
            profile::delete_profile_access_token,
        )
        .router;

    let routes = Router::new()
        .merge(bid::get_routes(store.clone()))
        .merge(opportunity::get_routes(store.clone()))
        .merge(profile_routes)
        .merge(ws::get_routes(store.clone()));

    let (prometheus_layer, _) = PrometheusMetricLayerBuilder::new()
        .with_metrics_from_fn(|| store.store.metrics_recorder.clone())
        .with_endpoint_label_type(EndpointLabel::MatchedPathWithFallbackFn(|_| {
            "unknown".to_string()
        }))
        .build_pair();

    // The generated OpenAPI document contains "discriminator" fields which are not generated correctly to be supported by redoc
    // We need to remove them from the document to make sure redoc can render the document correctly
    let original_doc = serde_json::to_value(ApiDoc::openapi())
        .expect("Failed to serialize OpenAPI document to json value");
    let mut redoc_doc = original_doc.clone();
    remove_discriminators(&mut redoc_doc);

    let app: Router<()> = Router::new()
        .merge(Redoc::with_url(Route::Docs.as_ref(), redoc_doc.clone()))
        .merge(routes)
        .route(Route::Root.as_ref(), get(root))
        .route(Route::Liveness.as_ref(), get(live))
        .route(Route::OpenApi.as_ref(), get(original_doc.to_string()))
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
