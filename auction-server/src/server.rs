use {
    crate::{
        api::{
            self,
            ws,
        },
        auction::run_submission_loop,
        config::{
            ChainId,
            Config,
            RunOptions,
        },
        opportunity_adapter::{
            get_eip_712_domain,
            get_weth_address,
            run_verification_loop,
        },
        state::{
            ChainStore,
            OpportunityStore,
            Store,
        },
    },
    anyhow::anyhow,
    ethers::{
        prelude::{
            LocalWallet,
            Provider,
        },
        providers::{
            Http,
            Middleware,
        },
        signers::Signer,
    },
    futures::{
        future::join_all,
        Future,
    },
    sqlx::postgres::PgPoolOptions,
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
    tokio::time::sleep,
};

async fn fault_tolerant_handler<F, Fut>(name: &str, f: F)
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


const NOTIFICATIONS_CHAN_LEN: usize = 1000;
pub async fn start_server(run_options: RunOptions) -> anyhow::Result<()> {
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

    let wallet = run_options.relayer_private_key.parse::<LocalWallet>()?;
    tracing::info!("Using wallet address: {}", wallet.address().to_string());

    let chain_store: anyhow::Result<HashMap<ChainId, ChainStore>> = join_all(
        config
            .chains
            .iter()
            .map(|(chain_id, chain_config)| async move {
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
                let weth =
                    get_weth_address(chain_config.opportunity_adapter_contract, provider.clone())
                        .await?;
                let eip_712_domain =
                    get_eip_712_domain(provider.clone(), chain_config.opportunity_adapter_contract)
                        .await
                        .map_err(|err| {
                            anyhow!(
                                "Failed to get domain separator for chain({chain_id}): {:?}",
                                err,
                                chain_id = chain_id
                            )
                        })?;

                Ok((
                    chain_id.clone(),
                    ChainStore {
                        provider,
                        network_id: id,
                        token_spoof_info: Default::default(),
                        config: chain_config.clone(),
                        weth,
                        eip_712_domain,
                    },
                ))
            }),
    )
    .await
    .into_iter()
    .collect();

    let (broadcast_sender, broadcast_receiver) =
        tokio::sync::broadcast::channel(NOTIFICATIONS_CHAN_LEN);

    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&run_options.server.database_url)
        .await
        .expect("Server should start with a valid database connection.");
    let store = Arc::new(Store {
        db:                pool,
        bids:              Default::default(),
        chains:            chain_store?,
        opportunity_store: OpportunityStore::default(),
        event_sender:      broadcast_sender.clone(),
        relayer:           wallet,
        ws:                ws::WsState {
            subscriber_counter: AtomicUsize::new(0),
            broadcast_sender,
            broadcast_receiver,
        },
    });

    let ss = String::from("development");
    tokio::join!(
        fault_tolerant_handler("submission loop", || run_submission_loop(
            store.clone(),
            ss.clone()
        )),
        fault_tolerant_handler("verification loop", || run_verification_loop(store.clone())),
        fault_tolerant_handler("start api", || api::start_api(
            run_options.clone(),
            store.clone()
        )),
    );
    Ok(())
}

// A static exit flag to indicate to running threads that we're shutting down. This is used to
// gracefully shutdown the application.
//
// NOTE: A more idiomatic approach would be to use a tokio::sync::broadcast channel, and to send a
// shutdown signal to all running tasks. However, this is a bit more complicated to implement and
// we don't rely on global state for anything else.
pub(crate) static SHOULD_EXIT: AtomicBool = AtomicBool::new(false);
pub const EXIT_CHECK_INTERVAL: Duration = Duration::from_secs(1);
