use {
    crate::{
        api::{
            self,
            ws,
        },
        auction::{
            get_express_relay_contract,
            run_log_listener_loop_svm,
            run_submission_loop,
            run_tracker_loop,
        },
        config::{
            ChainId,
            Config,
            ConfigEvm,
            ConfigMap,
            RunOptions,
        },
        models,
        opportunity::{
            service as opportunity_service,
            workers::run_verification_loop,
        },
        per_metrics,
        state::{
            ChainStoreCoreFields,
            ChainStoreEvm,
            ChainStoreSvm,
            ExpressRelaySvm,
            SimulatedBidEvm,
            SimulatedBidSvm,
            Store,
            StoreNew,
        },
        traced_client::TracedClient,
        traced_sender_svm::TracedSenderSvm,
    },
    anyhow::anyhow,
    axum_prometheus::{
        metrics_exporter_prometheus::{
            PrometheusBuilder,
            PrometheusHandle,
        },
        utils::SECONDS_DURATION_BUCKETS,
    },
    ethers::{
        core::k256::ecdsa::SigningKey,
        prelude::{
            LocalWallet,
            Provider,
        },
        providers::Middleware,
        signers::{
            Signer,
            Wallet,
        },
        types::BlockNumber,
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
    Fut: Future<Output = anyhow::Result<()>> + Send + 'static,
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

pub fn setup_metrics_recorder() -> anyhow::Result<PrometheusHandle> {
    PrometheusBuilder::new()
        .set_buckets(SECONDS_DURATION_BUCKETS)
        .unwrap()
        .install_recorder()
        .map_err(|err| anyhow!("Failed to set up metrics recorder: {:?}", err))
}


async fn setup_chain_store(
    config_map: ConfigMap,
    wallet: Wallet<SigningKey>,
) -> anyhow::Result<HashMap<ChainId, ChainStoreEvm>> {
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
                        let provider = get_chain_provider(&chain_id, &chain_config)?;

                        let id = provider.get_chainid().await?.as_u64();
                        let block = provider
                            .get_block(BlockNumber::Latest)
                            .await?
                            .expect("Failed to get latest block");

                        let express_relay_contract = get_express_relay_contract(
                            chain_config.express_relay_contract,
                            provider.clone(),
                            wallet.clone(),
                            chain_config.legacy_tx,
                            id,
                        );

                        Ok((
                            chain_id.clone(),
                            ChainStoreEvm {
                                name: chain_id.clone(),
                                core_fields: ChainStoreCoreFields::<SimulatedBidEvm> {
                                    bids:               Default::default(),
                                    auction_lock:       Default::default(),
                                    submitted_auctions: Default::default(),
                                },
                                provider,
                                network_id: id,
                                config: chain_config.clone(),
                                express_relay_contract: Arc::new(express_relay_contract),
                                block_gas_limit: block.gas_limit,
                            },
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
pub async fn start_server(run_options: RunOptions) -> anyhow::Result<()> {
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

    let chains = setup_chain_store(config_map.clone(), wallet.clone()).await?;

    let chains_svm = setup_svm(&run_options, config_map)?;

    let (broadcast_sender, broadcast_receiver) =
        tokio::sync::broadcast::channel(NOTIFICATIONS_CHAN_LEN);

    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&run_options.server.database_url)
        .await
        .expect("Server should start with a valid database connection.");
    match migrate!("./migrations").run(&pool).await {
        Ok(()) => {}
        Err(err) => match err {
            sqlx::migrate::MigrateError::VersionMissing(version) => {
                tracing::info!(
                    "Found missing migration ({}) probably because of downgrade",
                    version
                );
            }
            _ => {
                return Err(anyhow!("Failed to run migrations: {:?}", err));
            }
        },
    }
    let task_tracker = TaskTracker::new();

    let config_opportunity_service_evm =
        opportunity_service::ConfigEvm::from_chains(&chains).await?;
    let config_opportunity_service_svm =
        opportunity_service::ConfigSvm::from_chains(&chains_svm).await?;

    let chains = chains
        .into_iter()
        .map(|(k, v)| (k, Arc::new(v)))
        .collect::<HashMap<_, _>>();
    let chains_svm = chains_svm
        .into_iter()
        .map(|(k, v)| (k, Arc::new(v)))
        .collect::<HashMap<_, _>>();

    let access_tokens = fetch_access_tokens(&pool).await;
    let store = Arc::new(Store {
        db: pool.clone(),
        chains,
        chains_svm,
        event_sender: broadcast_sender.clone(),
        ws: ws::WsState {
            subscriber_counter: AtomicUsize::new(0),
            broadcast_sender,
            broadcast_receiver,
        },
        task_tracker: task_tracker.clone(),
        secret_key: run_options.secret_key.clone(),
        access_tokens: RwLock::new(access_tokens),
        metrics_recorder: setup_metrics_recorder()?,
    });

    let store_new = Arc::new(StoreNew {
        store:                   store.clone(),
        opportunity_service_evm: Arc::new(opportunity_service::Service::<
            opportunity_service::ChainTypeEvm,
        >::new(
            store.clone(),
            pool.clone(),
            config_opportunity_service_evm,
        )),
        opportunity_service_svm: Arc::new(opportunity_service::Service::<
            opportunity_service::ChainTypeSvm,
        >::new(
            store.clone(),
            pool.clone(),
            config_opportunity_service_svm,
        )),
    });

    tokio::join!(
        async {
            let submission_loops = store.chains.iter().map(|(chain_id, chain_store)| {
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
                    || run_log_listener_loop_svm(store_new.clone(), chain_store.clone()),
                )
            });
            join_all(log_listener_loops).await;
        },
        async {
            let tracker_loops = store.chains.iter().map(|(chain_id, chain_store)| {
                fault_tolerant_handler(
                    format!("tracker loop for chain {}", chain_id.clone()),
                    || run_tracker_loop(store.clone(), chain_store.clone()),
                )
            });
            join_all(tracker_loops).await;
        },
        fault_tolerant_handler("verification loop".to_string(), || run_verification_loop(
            store_new.opportunity_service_evm.clone()
        )),
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

fn setup_svm(
    run_options: &RunOptions,
    config_map: ConfigMap,
) -> anyhow::Result<HashMap<ChainId, ChainStoreSvm>> {
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
            let express_relay_svm = ExpressRelaySvm {
                relayer:                     relayer.clone(),
                permission_account_position: env!("SUBMIT_BID_PERMISSION_ACCOUNT_POSITION")
                    .parse::<usize>()
                    .expect("Failed to parse permission account position"),
                router_account_position:     env!("SUBMIT_BID_ROUTER_ACCOUNT_POSITION")
                    .parse::<usize>()
                    .expect("Failed to parse router account position"),
            };

            (
                chain_id.clone(),
                ChainStoreSvm {
                    name: chain_id.clone(),
                    core_fields: ChainStoreCoreFields::<SimulatedBidSvm> {
                        bids:               Default::default(),
                        auction_lock:       Default::default(),
                        submitted_auctions: Default::default(),
                    },
                    client: TracedSenderSvm::new_client(
                        chain_id.clone(),
                        chain_config.rpc_addr.as_str(),
                        chain_config.rpc_timeout,
                        RpcClientConfig::with_commitment(CommitmentConfig::processed()),
                    ),
                    wallet_program_router_account: chain_config.wallet_program_router_account,
                    config: chain_config,
                    express_relay_svm,
                },
            )
        })
        .collect())
}

pub fn get_chain_provider(
    chain_id: &String,
    chain_config: &ConfigEvm,
) -> anyhow::Result<Provider<TracedClient>> {
    let mut provider = TracedClient::new(
        chain_id.clone(),
        &chain_config.geth_rpc_addr,
        chain_config.rpc_timeout,
    )
    .map_err(|err| {
        tracing::error!(
            "Failed to create provider for chain({chain_id}) at {rpc_addr}: {:?}",
            err,
            chain_id = chain_id,
            rpc_addr = chain_config.geth_rpc_addr
        );
        anyhow!(
            "Failed to connect to chain({chain_id}) at {rpc_addr}: {:?}",
            err,
            chain_id = chain_id,
            rpc_addr = chain_config.geth_rpc_addr
        )
    })?;
    provider.set_interval(Duration::from_secs(chain_config.poll_interval));
    Ok(provider)
}

// A static exit flag to indicate to running threads that we're shutting down. This is used to
// gracefully shutdown the application.
//
// NOTE: A more idiomatic approach would be to use a tokio::sync::broadcast channel, and to send a
// shutdown signal to all running tasks. However, this is a bit more complicated to implement and
// we don't rely on global state for anything else.
pub(crate) static SHOULD_EXIT: AtomicBool = AtomicBool::new(false);
pub const EXIT_CHECK_INTERVAL: Duration = Duration::from_secs(1);
