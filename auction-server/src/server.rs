use {
    crate::{
        api::{
            self,
            ws,
        },
        auction_old::{
            run_auction_conclusion_loop_svm, run_log_listener_loop_svm, run_submission_loop, run_tracker_loop
        },
        bid::service as bid_service,
        config::{
            ChainId,
            Config,
            ConfigMap,
            MigrateOptions,
            RunOptions,
        },
        kernel::{
            entities::ChainType,
            traced_sender_svm::TracedSenderSvm,
        },
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
        watcher::run_watcher_loop_svm,
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
        core::k256::ecdsa::SigningKey,
        prelude::LocalWallet,
        signers::{
            Signer,
            Wallet,
        },
    },
    futures::{
        future::join_all,
        Future,
    },
    solana_client::rpc_client::RpcClientConfig,
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
        default::Default,
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

pub fn setup_metrics_recorder() -> Result<PrometheusHandle> {
    PrometheusBuilder::new()
        .set_buckets(SECONDS_DURATION_BUCKETS)
        .unwrap()
        .install_recorder()
        .map_err(|err| anyhow!("Failed to set up metrics recorder: {:?}", err))
}


async fn setup_chain_store_evm(
    config_map: ConfigMap,
    wallet: Wallet<SigningKey>,
) -> Result<HashMap<ChainId, ChainStoreEvm>> {
    join_all(
        config_map
            .chains
            .iter()
            .filter_map(|(chain_id, config)| match config {
                Config::Svm(_) => None,
                Config::Evm(chain_config) => {
                    let (chain_id, chain_config, wallet) =
                        (chain_id.clone(), chain_config.clone(), wallet.clone());
                    Some(async move {
                        Ok((
                            chain_id.clone(),
                            ChainStoreEvm::create_store(chain_id, chain_config, wallet).await?,
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

    let chains_evm = setup_chain_store_evm(config_map.clone(), wallet.clone()).await?;

    let chains_svm = setup_chain_store_svm(&run_options, config_map)?;

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
        event_sender:     broadcast_sender.clone(),
        ws:               ws::WsState {
            subscriber_counter: AtomicUsize::new(0),
            broadcast_sender,
            broadcast_receiver,
        },
        task_tracker:     task_tracker.clone(),
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
    let mut bid_services: HashMap<ChainId, bid_service::ServiceEnum> = chains_evm
        .iter()
        .map(|(chain_id, chain_store)| {
            (
                chain_id.clone(),
                bid_service::ServiceEnum::Evm(Arc::new(bid_service::Service::new(
                    pool.clone(),
                    bid_service::Config {
                        chain_type:   ChainType::Evm,
                        chain_id:     chain_id.clone(),
                        chain_config: bid_service::ConfigEvm::new(
                            bid_service::ExpressRelayEvm {
                                relayer:          wallet.clone(),
                                contract_address: chain_store.config.express_relay_contract,
                            },
                            chain_store.provider.clone(),
                            chain_store.block_gas_limit,
                        ),
                    },
                    opportunity_service_evm.clone(),
                ))),
            )
        })
        .collect();
    chains_svm.iter().for_each(|(chain_id, chain_store)| {
        if bid_services
            .insert(
                chain_id.clone(),
                bid_service::ServiceEnum::Svm(Arc::new(bid_service::Service::new(
                    pool.clone(),
                    bid_service::Config {
                        chain_type:   ChainType::Svm,
                        chain_id:     chain_id.clone(),
                        chain_config: bid_service::ConfigSvm {
                            client:                        TracedSenderSvm::new_client(
                                chain_id.clone(),
                                chain_store.config.rpc_read_url.as_str(),
                                chain_store.config.rpc_timeout,
                                RpcClientConfig::with_commitment(CommitmentConfig::processed()),
                            ),
                            wallet_program_router_account: chain_store
                                .config
                                .wallet_program_router_account,
                            express_relay:                 bid_service::ExpressRelaySvm {
                                program_id:                  chain_store
                                    .config
                                    .express_relay_program_id,
                                relayer:                     Keypair::from_base58_string(
                                    &run_options
                                        .private_key_svm
                                        .clone()
                                        .expect("No svm private key provided for chain"),
                                ),
                                permission_account_position: chain_store
                                    .express_relay_svm
                                    .permission_account_position,
                                router_account_position:     chain_store
                                    .express_relay_svm
                                    .router_account_position,
                            },
                        },
                    },
                    opportunity_service_svm.clone(),
                ))),
            )
            .is_some()
        {
            panic!("Duplicate chain id: {}", chain_id);
        }
    });

    let store_new = Arc::new(StoreNew::new(
        store.clone(),
        opportunity_service_evm,
        opportunity_service_svm,
        bid_services,
    ));

    tokio::join!(
        async {
            let submission_loops = store.chains_evm.iter().map(|(chain_id, chain_store)| {
                fault_tolerant_handler(
                    format!("submission loop for evm chain {}", chain_id.clone()),
                    || run_submission_loop(store_new.clone(), chain_store.clone()),
                )
            });
            join_all(submission_loops).await;
        },
        async {
            let submission_loops = store.chains_svm.iter().map(|(chain_id, chain_store)| {
                fault_tolerant_handler(
                    format!("submission loop for svm chain {}", chain_id.clone()),
                    || run_submission_loop(store_new.clone(), chain_store.clone()),
                )
            });
            join_all(submission_loops).await;
        },
        async {
            let log_listener_loops = store.chains_svm.iter().map(|(chain_id, chain_store)| {
                fault_tolerant_handler(
                    format!("log listener loop for svm chain {}", chain_id.clone()),
                    || run_log_listener_loop_svm(chain_store.clone()),
                )
            });
            join_all(log_listener_loops).await;
        },
        async {
            let log_listener_loops = store.chains_svm.iter().map(|(chain_id, chain_store)| {
                fault_tolerant_handler(
                    format!("conclusion loop for svm chain {}", chain_id.clone()),
                    || run_auction_conclusion_loop_svm(store_new.clone(), chain_store.clone()),
                )
            });
            join_all(log_listener_loops).await;
        },
        async {
            let tracker_loops = store.chains_evm.iter().map(|(chain_id, chain_store)| {
                fault_tolerant_handler(
                    format!("tracker loop for chain {}", chain_id.clone()),
                    || run_tracker_loop(chain_store.clone()),
                )
            });
            join_all(tracker_loops).await;
        },
        async {
            let watcher_loops = store.chains_svm.keys().map(|chain_id| {
                fault_tolerant_handler(
                    format!("watcher loop for chain {}", chain_id.clone()),
                    || run_watcher_loop_svm(store.clone(), chain_id.clone()),
                )
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

fn setup_chain_store_svm(
    run_options: &RunOptions,
    config_map: ConfigMap,
) -> Result<HashMap<ChainId, ChainStoreSvm>> {
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
    let relayer = Arc::new(Keypair::from_base58_string(
        &run_options
            .private_key_svm
            .clone()
            .ok_or(anyhow!("No svm private key provided for svm chains"))?,
    ));
    Ok(svm_chains
        .into_iter()
        .map(|(chain_id, chain_config)| {
            (
                chain_id.clone(),
                ChainStoreSvm::new(chain_id.clone(), chain_config.clone(), relayer.clone()),
            )
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
