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
        },
        auction::run_submission_loop,
        config::{
            ChainId,
            Config,
            RunOptions,
        },
        liquidation_adapter::run_verification_loop,
        state::{
            ChainStore,
            LiquidationStore,
            OpportunityParams,
            OpportunityParamsV1,
            Store,
            TokenQty,
        },
    },
    anyhow::{
        anyhow,
        Result,
    },
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
    ethers::{
        providers::{
            Http,
            Middleware,
            Provider,
        },
        signers::{
            LocalWallet,
            Signer,
        },
        types::Bytes,
    },
    futures::future::join_all,
    serde::Serialize,
    std::{
        collections::HashMap,
        sync::{
            atomic::{
                AtomicBool,
                AtomicUsize,
                Ordering,
            },
            Arc,
        },
        time::Duration,
    },
    tower_http::cors::CorsLayer,
    utoipa::{
        OpenApi,
        ToResponse,
        ToSchema,
    },
    utoipa_swagger_ui::SwaggerUi,
};

// A static exit flag to indicate to running threads that we're shutting down. This is used to
// gracefully shutdown the application.
//
// NOTE: A more idiomatic approach would be to use a tokio::sync::broadcast channel, and to send a
// shutdown signal to all running tasks. However, this is a bit more complicated to implement and
// we don't rely on global state for anything else.
pub(crate) static SHOULD_EXIT: AtomicBool = AtomicBool::new(false);
pub const EXIT_CHECK_INTERVAL: Duration = Duration::from_secs(1);
const NOTIFICATIONS_CHAN_LEN: usize = 1000;
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

pub async fn start_server(run_options: RunOptions) -> Result<()> {
    tokio::spawn(async move {
        tracing::info!("Registered shutdown signal handler...");
        tokio::signal::ctrl_c().await.unwrap();
        tracing::info!("Shut down signal received, waiting for tasks...");
        SHOULD_EXIT.store(true, Ordering::Release);
    });


    let config = Config::load(&run_options.config.config).map_err(|err| {
        anyhow!(
            "Failed to load config from file({path}): {:?}",
            err,
            path = run_options.config.config
        )
    })?;

    let wallet = run_options.per_private_key.parse::<LocalWallet>()?;
    tracing::info!("Using wallet address: {}", wallet.address().to_string());

    let chain_store: Result<HashMap<ChainId, ChainStore>> = join_all(config.chains.iter().map(
        |(chain_id, chain_config)| async move {
            let mut provider = Provider::<Http>::try_from(chain_config.geth_rpc_addr.clone())
                .map_err(|err| {
                    anyhow!(
                        "Failed to connect to chain({chain_id}) at {rpc_addr}: {:?}",
                        err,
                        chain_id = chain_id,
                        rpc_addr = chain_config.geth_rpc_addr
                    )
                })?;
            provider.set_interval(Duration::from_secs(chain_config.poll_interval));
            let id = provider.get_chainid().await?.as_u64();
            Ok((
                chain_id.clone(),
                ChainStore {
                    provider,
                    network_id: id,
                    bids: Default::default(),
                    token_spoof_info: Default::default(),
                    config: chain_config.clone(),
                },
            ))
        },
    ))
    .await
    .into_iter()
    .collect();

    let (update_tx, update_rx) = tokio::sync::broadcast::channel(NOTIFICATIONS_CHAN_LEN);
    let store = Arc::new(Store {
        chains:            chain_store?,
        liquidation_store: LiquidationStore::default(),
        per_operator:      wallet,
        ws:                ws::WsState {
            subscriber_counter: AtomicUsize::new(0),
            broadcast_sender:   update_tx,
            broadcast_receiver: update_rx,
        },
    });


    let submission_loop = tokio::spawn(run_submission_loop(store.clone()));
    let verification_loop = tokio::spawn(run_verification_loop(store.clone()));
    let server_loop = tokio::spawn(start_api(run_options, store.clone()));
    join_all(vec![submission_loop, verification_loop, server_loop]).await;
    Ok(())
}
