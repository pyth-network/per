use {
    crate::{
        api::{
            self,
            ws,
        },
        auction::service::{
            self as auction_service,
            simulator::Simulator,
            SubmitBidInstructionAccountPositions,
            SwapInstructionAccountPositions,
        },
        config::{
            ChainId,
            Config,
            ConfigMap,
            MigrateOptions,
            RunOptions,
        },
        kernel::traced_sender_svm::TracedSenderSvm,
        models,
        opportunity::{
            service as opportunity_service,
            workers::run_verification_loop,
        },
        per_metrics,
        state::{
            ChainStoreEvm,
            ChainStoreSvm,
            Store,
            StoreNew,
        },
    },
    anyhow::{
        anyhow,
        Result,
    },
    axum_prometheus::{
        metrics_exporter_prometheus::{
            PrometheusBuilder,
            PrometheusHandle,
        },
        utils::SECONDS_DURATION_BUCKETS,
    },
    ethers::{
        prelude::LocalWallet,
        signers::Signer,
    },
    futures::{
        future::join_all,
        Future,
    },
    solana_client::{
        nonblocking::rpc_client::RpcClient,
        rpc_client::RpcClientConfig,
    },
    solana_sdk::{
        commitment_config::CommitmentConfig,
        signature::Keypair,
    },
    sqlx::{
        migrate,
        postgres::PgPoolOptions,
        PgPool,
    },
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
    tokio::{
        sync::RwLock,
        time::sleep,
    },
    tokio_util::task::TaskTracker,
};

async fn fault_tolerant_handler<F, Fut>(name: String, f: F)
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<()>> + Send + 'static,
    Fut::Output: Send + 'static,
{
    loop {
        let res = tokio::spawn(f()).await;
        match res {
            Ok(result) => match result {
                Ok(_) => break, // This will happen on graceful shutdown
                Err(err) => {
                    tracing::error!("{} returned error: {:?}", name, err);
                    sleep(Duration::from_millis(500)).await;
                }
            },
            Err(err) => {
                tracing::error!("{} is panicked or canceled: {:?}", name, err);
                SHOULD_EXIT.store(true, Ordering::Release);
                break;
            }
        }
    }
}

async fn fetch_access_tokens(db: &PgPool) -> HashMap<models::AccessTokenToken, models::Profile> {
    let access_tokens = sqlx::query_as!(
        models::AccessToken,
        "SELECT * FROM access_token WHERE revoked_at IS NULL",
    )
    .fetch_all(db)
    .await
    .expect("Failed to fetch access tokens from database");
    let profile_ids: Vec<models::ProfileId> =
        access_tokens.iter().map(|token| token.profile_id).collect();
    let profiles: Vec<models::Profile> = sqlx::query_as("SELECT * FROM profile WHERE id = ANY($1)")
        .bind(profile_ids)
        .fetch_all(db)
        .await
        .expect("Failed to fetch profiles from database");

    access_tokens
        .into_iter()
        .map(|token| {
            let profile = profiles
                .iter()
                .find(|profile| profile.id == token.profile_id)
                .expect("Profile not found");
            (token.token, profile.clone())
        })
        .collect()
}

pub const TRANSACTION_LANDING_TIME_SVM_METRIC: &str = "transaction_landing_time_seconds_svm";
const TRANSACTION_LANDING_TIME_SVM_BUCKETS: &[f64; 14] = &[
    0.1, 0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0, 2.25, 2.5, 3.75, 5.0, 10.0,
];

pub fn setup_metrics_recorder() -> Result<PrometheusHandle> {
    PrometheusBuilder::new()
        .set_buckets_for_metric(
            axum_prometheus::metrics_exporter_prometheus::Matcher::Full(
                TRANSACTION_LANDING_TIME_SVM_METRIC.to_string(),
            ),
            TRANSACTION_LANDING_TIME_SVM_BUCKETS,
        )
        .unwrap()
        .set_buckets(SECONDS_DURATION_BUCKETS)
        .unwrap()
        .install_recorder()
        .map_err(|err| anyhow!("Failed to set up metrics recorder: {:?}", err))
}

async fn setup_chain_store_evm(config_map: ConfigMap) -> Result<HashMap<ChainId, ChainStoreEvm>> {
    join_all(
        config_map
            .chains
            .iter()
            .filter_map(|(chain_id, config)| match config {
                Config::Svm(_) => None,
                Config::Evm(chain_config) => {
                    let (chain_id, chain_config) = (chain_id.clone(), chain_config.clone());
                    Some(async move {
                        Ok((
                            chain_id.clone(),
                            ChainStoreEvm::create_store(chain_id, chain_config).await?,
                        ))
                    })
                }
            }),
    )
    .await
    .into_iter()
    .collect()
}

const NOTIFICATIONS_CHAN_LEN: usize = 1000;

// TODO move to kernel repo
async fn create_pg_pool(database_url: &str) -> Result<PgPool> {
    PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await
        .map_err(|err| anyhow!("Failed to connect to database: {:?}", err))
}

pub async fn run_migrations(migrate_options: MigrateOptions) -> Result<()> {
    let pool = create_pg_pool(&migrate_options.database_url).await?;
    let migrator = migrate!("./migrations");
    if let Err(err) = migrator.run(&pool).await {
        match err {
            sqlx::migrate::MigrateError::VersionMissing(version) => {
                tracing::info!(
                    "Found missing migration ({}) probably because of downgrade",
                    version
                );
            }
            _ => {
                return Err(anyhow!("Failed to run migrations: {:?}", err));
            }
        }
    }
    tracing::info!("Migrations ran successfully");
    let last_migration_desc = migrator
        .iter()
        .last()
        .map(|x| x.description.as_ref())
        .unwrap_or("No migrations found");
    tracing::info!("Last migration: {}", last_migration_desc);
    Ok(())
}

macro_rules! read_svm_position_env {
    ($name:expr) => {{
        // Access the environment variable at compile-time
        let value = env!($name); // We expect $name to be a string literal

        // Parse the value to usize
        value.parse::<usize>().expect(&format!(
            "Failed to parse the environment variable {:?} as usize",
            $name
        ))
    }};
}

fn get_swap_instruction_account_positions() -> SwapInstructionAccountPositions {
    SwapInstructionAccountPositions {
        router_token_account:   read_svm_position_env!("SWAP_ROUTER_TOKEN_ACCOUNT_POSITION"),
        user_wallet_account:    read_svm_position_env!("SWAP_USER_WALLET_ACCOUNT_POSITION"),
        mint_searcher_account:  read_svm_position_env!("SWAP_MINT_SEARCHER_ACCOUNT_POSITION"),
        mint_user_account:      read_svm_position_env!("SWAP_MINT_USER_ACCOUNT_POSITION"),
        token_program_searcher: read_svm_position_env!("SWAP_TOKEN_PROGRAM_SEARCHER_POSITION"),
        token_program_user:     read_svm_position_env!("SWAP_TOKEN_PROGRAM_USER_POSITION"),
    }
}

fn get_submit_bid_instruction_account_positions() -> SubmitBidInstructionAccountPositions {
    SubmitBidInstructionAccountPositions {
        permission_account: read_svm_position_env!("SUBMIT_BID_PERMISSION_ACCOUNT_POSITION"),
        router_account:     read_svm_position_env!("SUBMIT_BID_ROUTER_ACCOUNT_POSITION"),
    }
}

pub async fn start_server(run_options: RunOptions) -> Result<()> {
    tokio::spawn(async move {
        tracing::info!("Registered shutdown signal handler...");
        tokio::signal::ctrl_c().await.unwrap();
        tracing::info!("Shut down signal received, waiting for tasks...");
        SHOULD_EXIT.store(true, Ordering::Release);
    });

    let config_map = ConfigMap::load(&run_options.config.config).map_err(|err| {
        anyhow!(
            "Failed to load config from file({path}): {:?}",
            err,
            path = run_options.config.config
        )
    })?;

    let wallet = run_options.subwallet_private_key.parse::<LocalWallet>()?;
    tracing::info!("Using wallet address: {:?}", wallet.address());

    let chains_evm = setup_chain_store_evm(config_map.clone()).await?;

    let chains_svm = setup_chain_store_svm(config_map)?;

    let (broadcast_sender, broadcast_receiver) =
        tokio::sync::broadcast::channel(NOTIFICATIONS_CHAN_LEN);

    let pool = create_pg_pool(&run_options.server.database_url).await?;
    let task_tracker = TaskTracker::new();

    let config_opportunity_service_evm =
        opportunity_service::ConfigEvm::from_chains(&chains_evm).await?;
    let config_opportunity_service_svm =
        opportunity_service::ConfigSvm::from_chains(&chains_svm).await?;

    let chains_evm = chains_evm
        .into_iter()
        .map(|(k, v)| (k, Arc::new(v)))
        .collect::<HashMap<_, _>>();
    let chains_svm = chains_svm
        .into_iter()
        .map(|(k, v)| (k, Arc::new(v)))
        .collect::<HashMap<_, _>>();

    let access_tokens = fetch_access_tokens(&pool).await;
    let store = Arc::new(Store {
        db:               pool.clone(),
        chains_evm:       chains_evm.clone(),
        chains_svm:       chains_svm.clone(),
        ws:               ws::WsState {
            subscriber_counter: AtomicUsize::new(0),
            broadcast_sender,
            broadcast_receiver,
        },
        secret_key:       run_options.secret_key.clone(),
        access_tokens:    RwLock::new(access_tokens),
        metrics_recorder: setup_metrics_recorder()?,
    });

    let opportunity_service_evm = Arc::new(opportunity_service::Service::<
        opportunity_service::ChainTypeEvm,
    >::new(
        store.clone(),
        pool.clone(),
        config_opportunity_service_evm,
    ));
    let opportunity_service_svm = Arc::new(opportunity_service::Service::<
        opportunity_service::ChainTypeSvm,
    >::new(
        store.clone(),
        pool.clone(),
        config_opportunity_service_svm,
    ));
    #[allow(clippy::iter_kv_map)]
    let mut auction_services: HashMap<ChainId, auction_service::ServiceEnum> = chains_evm
        .iter()
        .map(|(chain_id, chain_store)| {
            (
                chain_id.clone(),
                auction_service::ServiceEnum::Evm(auction_service::Service::new(
                    pool.clone(),
                    auction_service::Config {
                        chain_id:     chain_id.clone(),
                        chain_config: auction_service::ConfigEvm::new(
                            wallet.clone(),
                            chain_store.config.express_relay_contract,
                            chain_store.provider.clone(),
                            chain_store.block_gas_limit,
                            chain_store.config.geth_ws_addr.clone(),
                            chain_store.network_id,
                        ),
                    },
                    opportunity_service_evm.clone(),
                    task_tracker.clone(),
                    store.ws.broadcast_sender.clone(),
                )),
            )
        })
        .collect();
    chains_svm.iter().for_each(|(chain_id, chain_store)| {
        let tx_broadcaster_clients: Vec<RpcClient> = chain_store
            .config
            .rpc_tx_submission_urls
            .iter()
            .map(|url| {
                TracedSenderSvm::new_client(
                    chain_id.clone(),
                    url.as_str(),
                    chain_store.config.rpc_timeout,
                    RpcClientConfig::with_commitment(CommitmentConfig::processed()),
                )
            })
            .collect();
        if tx_broadcaster_clients.is_empty() {
            panic!("No tx broadcaster client provided for chain {}", chain_id);
        }
        if auction_services
            .insert(
                chain_id.clone(),
                auction_service::ServiceEnum::Svm(auction_service::Service::new(
                    pool.clone(),
                    auction_service::Config {
                        chain_id:     chain_id.clone(),
                        chain_config: auction_service::ConfigSvm {
                            client: TracedSenderSvm::new_client(
                                chain_id.clone(),
                                chain_store.config.rpc_read_url.as_str(),
                                chain_store.config.rpc_timeout,
                                RpcClientConfig::with_commitment(CommitmentConfig::processed()),
                            ),
                            simulator: Simulator::new(TracedSenderSvm::new_client(
                                chain_id.clone(),
                                chain_store.config.rpc_read_url.as_str(),
                                chain_store.config.rpc_timeout,
                                RpcClientConfig::with_commitment(CommitmentConfig::processed()),
                            )),
                            express_relay: auction_service::ExpressRelaySvm {
                                program_id:                               chain_store
                                    .config
                                    .express_relay_program_id,
                                relayer:
                                    Keypair::from_base58_string(
                                        &run_options
                                            .private_key_svm
                                            .clone()
                                            .expect("No svm private key provided for chain"),
                                    ),
                                submit_bid_instruction_account_positions:
                                    get_submit_bid_instruction_account_positions(),
                                swap_instruction_account_positions:
                                    get_swap_instruction_account_positions(),
                            },
                            ws_address: chain_store.config.ws_addr.clone(),
                            tx_broadcaster_clients,
                            log_sender: chain_store.log_sender.clone(),
                            prioritization_fee_percentile: chain_store
                                .config
                                .prioritization_fee_percentile,
                            // _dummy_log_receiver: chain_store._dummy_log_receiver.clone(),
                        },
                    },
                    opportunity_service_svm.clone(),
                    task_tracker.clone(),
                    store.ws.broadcast_sender.clone(),
                )),
            )
            .is_some()
        {
            panic!("Duplicate chain id: {}", chain_id);
        }
    });

    for (chain_id, service) in auction_services.iter() {
        match service {
            auction_service::ServiceEnum::Evm(service) => {
                let config = opportunity_service_evm
                    .get_config(chain_id)
                    .expect("Failed to get opportunity service evm config");
                config.inject_auction_service(service.clone()).await;
            }
            auction_service::ServiceEnum::Svm(service) => {
                let config = opportunity_service_svm
                    .get_config(chain_id)
                    .expect("Failed to get opportunity service svm config");
                config.inject_auction_service(service.clone()).await;
            }
        }
    }

    let store_new = Arc::new(StoreNew::new(
        store.clone(),
        opportunity_service_evm,
        opportunity_service_svm,
        auction_services.clone(),
    ));

    tokio::join!(
        async {
            let submission_loops = auction_services.iter().filter_map(|(chain_id, service)| {
                if let auction_service::ServiceEnum::Evm(service) = service {
                    Some(fault_tolerant_handler(
                        format!("submission loop for chain {}", chain_id.clone()),
                        || {
                            let service = service.clone();
                            async move { service.run_submission_loop().await }
                        },
                    ))
                } else {
                    None
                }
            });
            join_all(submission_loops).await;
        },
        async {
            let submission_loops = auction_services.iter().filter_map(|(chain_id, service)| {
                if let auction_service::ServiceEnum::Svm(service) = service {
                    Some(fault_tolerant_handler(
                        format!("submission loop for chain {}", chain_id.clone()),
                        || {
                            let service = service.clone();
                            async move { service.run_submission_loop().await }
                        },
                    ))
                } else {
                    None
                }
            });
            join_all(submission_loops).await;
        },
        async {
            let log_listener_loops = auction_services.iter().filter_map(|(chain_id, service)| {
                if let auction_service::ServiceEnum::Svm(service) = service {
                    Some(fault_tolerant_handler(
                        format!("log listener loop for chain {}", chain_id.clone()),
                        || {
                            let service = service.clone();
                            async move { service.run_log_listener_loop().await }
                        },
                    ))
                } else {
                    None
                }
            });
            join_all(log_listener_loops).await;
        },
        async {
            let auction_conclusion_loops =
                auction_services.iter().filter_map(|(chain_id, service)| {
                    if let auction_service::ServiceEnum::Svm(service) = service {
                        Some(fault_tolerant_handler(
                            format!(
                                "auction conclusion loops loop for chain {}",
                                chain_id.clone()
                            ),
                            || {
                                let service = service.clone();
                                async move { service.run_auction_conclusion_loop().await }
                            },
                        ))
                    } else {
                        None
                    }
                });
            join_all(auction_conclusion_loops).await;
        },
        async {
            let tracker_loops = auction_services.iter().filter_map(|(chain_id, service)| {
                if let auction_service::ServiceEnum::Evm(service) = service {
                    Some(fault_tolerant_handler(
                        format!("tracker loop for chain {}", chain_id.clone()),
                        || {
                            let service = service.clone();
                            async move { service.run_tracker_loop().await }
                        },
                    ))
                } else {
                    None
                }
            });
            join_all(tracker_loops).await;
        },
        async {
            let watcher_loops = auction_services.iter().filter_map(|(chain_id, service)| {
                if let auction_service::ServiceEnum::Svm(service) = service {
                    Some(fault_tolerant_handler(
                        format!("watcher loop for chain {}", chain_id.clone()),
                        || {
                            let service = service.clone();
                            async move { service.run_watcher_loop().await }
                        },
                    ))
                } else {
                    None
                }
            });
            join_all(watcher_loops).await;
        },
        fault_tolerant_handler("evm verification loop".to_string(), || {
            run_verification_loop(store_new.opportunity_service_evm.clone())
        }),
        fault_tolerant_handler("svm verification loop".to_string(), || {
            run_verification_loop(store_new.opportunity_service_svm.clone())
        }),
        fault_tolerant_handler("start api".to_string(), || api::start_api(
            run_options.clone(),
            store_new.clone(),
        )),
        fault_tolerant_handler("start metrics".to_string(), || per_metrics::start_metrics(
            run_options.clone(),
            store.clone()
        )),
    );

    // To make sure all the spawned tasks will finish their job before shut down
    // Closing task tracker doesn't mean that it won't accept new tasks!!
    task_tracker.close();
    task_tracker.wait().await;

    Ok(())
}

fn setup_chain_store_svm(config_map: ConfigMap) -> Result<HashMap<ChainId, ChainStoreSvm>> {
    let svm_chains: Vec<_> = config_map
        .chains
        .iter()
        .filter_map(|(chain_id, config)| match config {
            Config::Evm(_) => None,
            Config::Svm(chain_config) => Some((chain_id.clone(), chain_config.clone())),
        })
        .collect();
    if svm_chains.is_empty() {
        return Ok(HashMap::new());
    }
    Ok(svm_chains
        .into_iter()
        .map(|(chain_id, chain_config)| {
            (chain_id.clone(), ChainStoreSvm::new(chain_config.clone()))
        })
        .collect())
}

// A static exit flag to indicate to running threads that we're shutting down. This is used to
// gracefully shutdown the application.
//
// NOTE: A more idiomatic approach would be to use a tokio::sync::broadcast channel, and to send a
// shutdown signal to all running tasks. However, this is a bit more complicated to implement and
// we don't rely on global state for anything else.
pub(crate) static SHOULD_EXIT: AtomicBool = AtomicBool::new(false);
pub const EXIT_CHECK_INTERVAL: Duration = Duration::from_secs(1);
