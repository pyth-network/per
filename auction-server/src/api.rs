use {
    crate::{
        api::{
            marketplace::{
                Order,
                OrderBid,
                TokenAmount,
            },
            rest::Bid,
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
        types::{
            Address,
            H160,
        },
    },
    futures::future::join_all,
    std::{
        collections::HashMap,
        sync::{
            atomic::{
                AtomicBool,
                Ordering,
            },
            Arc,
        },
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

pub(crate) mod marketplace;
mod rest;

#[derive(ToResponse, ToSchema)]
#[response(description = "An error occurred processing the request")]
pub enum RestError {
    /// The request contained invalid parameters
    BadParameters(String),
    /// The chain id is not supported
    InvalidChainId,
    /// The order was not found
    OrderNotFound,
    /// The server cannot currently communicate with the blockchain, so is not able to verify
    /// which random values have been requested.
    TemporarilyUnavailable,
    /// A catch-all error for all other types of errors that could occur during processing.
    Unknown,
}

impl IntoResponse for RestError {
    fn into_response(self) -> Response {
        match self {
            RestError::BadParameters(msg) => {
                (StatusCode::BAD_REQUEST, format!("Bad parameters: {}", msg)).into_response()
            }
            RestError::InvalidChainId => {
                (StatusCode::BAD_REQUEST, "The chain id is not supported").into_response()
            }
            RestError::OrderNotFound => (
                StatusCode::NOT_FOUND,
                "Order with the specified id was not found",
            )
                .into_response(),

            RestError::TemporarilyUnavailable => (
                StatusCode::SERVICE_UNAVAILABLE,
                "This service is temporarily unavailable",
            )
                .into_response(),
            RestError::Unknown => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "An unknown error occurred processing the request",
            )
                .into_response(),
        }
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
    rest::bid,
    marketplace::submit_order,
    marketplace::fetch_orders,
    ),
    components(
        schemas(Bid),schemas(Order),schemas(OrderBid), schemas(TokenAmount),responses(RestError)
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
            let provider =
                Provider::<Http>::try_from(chain_config.geth_rpc_addr.clone()).map_err(|err| {
                    anyhow!(
                        "Failed to connect to chain({chain_id}) at {rpc_addr}: {:?}",
                        err,
                        chain_id = chain_id,
                        rpc_addr = chain_config.geth_rpc_addr
                    )
                })?;
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
        .route("/bid", post(rest::bid))
        .route("/orders/submit_order", post(marketplace::submit_order))
        .route("/orders/fetch_orders", get(marketplace::fetch_orders))
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
