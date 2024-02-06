use {
    crate::{
        api::{
            bid::Bid,
            liquidation::{
                LiquidationOpportunity,
                OpportunityBid,
                TokenQty,
            },
        },
        auction::run_submission_loop,
        config::{
            ChainId,
            Config,
            RunOptions,
        },
        state::{
            ChainStore,
            LiquidationStore,
            Store,
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

async fn root() -> String {
    format!("PER Auction Server API {}", crate_version!())
}

mod bid;
pub(crate) mod liquidation;

pub enum RestError {
    /// The request contained invalid parameters
    BadParameters(String),
    /// The chain id is not supported
    InvalidChainId,
    /// The simulation failed
    SimulationError { result: Bytes, reason: String },
    /// The order was not found
    OpportunityNotFound,
    /// The server cannot currently communicate with the blockchain, so is not able to verify
    /// which random values have been requested.
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
                "Order with the specified id was not found".to_string(),
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

pub async fn start_server(run_options: RunOptions) -> Result<()> {
    tokio::spawn(async move {
        tracing::info!("Registered shutdown signal handler...");
        tokio::signal::ctrl_c().await.unwrap();
        tracing::info!("Shut down signal received, waiting for tasks...");
        SHOULD_EXIT.store(true, Ordering::Release);
    });

    #[derive(OpenApi)]
    #[openapi(
    paths(
    bid::bid,
    liquidation::submit_opportunity,
    liquidation::bid_opportunity,
    liquidation::fetch_opportunities,
    ),
    components(
    schemas(Bid),
    schemas(LiquidationOpportunity),
    schemas(OpportunityBid),
    schemas(TokenQty),
    schemas(ErrorBodyResponse),
    responses(ErrorBodyResponse)
    ),
    tags(
    (name = "PER Auction", description = "Pyth Express Relay Auction Server")
    )
    )]
    struct ApiDoc;

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
                    config: chain_config.clone(),
                },
            ))
        },
    ))
    .await
    .into_iter()
    .collect();

    let store = Arc::new(Store {
        chains:            chain_store?,
        liquidation_store: LiquidationStore::default(),
        per_operator:      wallet,
    });

    let server_store = store.clone();

    tokio::spawn(run_submission_loop(store.clone()));

    let app: Router<()> = Router::new()
        .merge(SwaggerUi::new("/docs").url("/docs/openapi.json", ApiDoc::openapi()))
        .route("/", get(root))
        .route("/v1/bid", post(bid::bid))
        .route(
            "/v1/liquidation/submit_opportunity",
            post(liquidation::submit_opportunity),
        )
        .route(
            "/v1/liquidation/fetch_opportunities",
            get(liquidation::fetch_opportunities),
        )
        .route(
            "/v1/liquidation/bid_opportunity",
            post(liquidation::bid_opportunity),
        )
        .layer(CorsLayer::permissive())
        .with_state(server_store);

    axum::Server::bind(&run_options.server.listen_addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(async {
            while !SHOULD_EXIT.load(Ordering::Acquire) {
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
            tracing::info!("Shutting down RPC server...");
        })
        .await?;

    Ok(())
}
