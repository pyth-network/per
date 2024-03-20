use {
    crate::{
        api,
        api::ws,
        auction::run_submission_loop,
        config::{
            ChainId,
            Config,
            RunOptions,
        },
        opportunity_adapter::run_verification_loop,
        state::{
            BidStatusStore,
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
    futures::future::join_all,
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
};

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
            }),
    )
    .await
    .into_iter()
    .collect();

    let (broadcast_sender, broadcast_receiver) =
        tokio::sync::broadcast::channel(NOTIFICATIONS_CHAN_LEN);
    let store = Arc::new(Store {
        chains:            chain_store?,
        opportunity_store: OpportunityStore::default(),
        bid_status_store:  BidStatusStore {
            bids_status:  Default::default(),
            event_sender: broadcast_sender.clone(),
        },
        relayer:           wallet,
        ws:                ws::WsState {
            subscriber_counter: AtomicUsize::new(0),
            broadcast_sender,
            broadcast_receiver,
        },
    });

    let submission_loop = tokio::spawn(run_submission_loop(store.clone()));
    let verification_loop = tokio::spawn(run_verification_loop(store.clone()));
    let server_loop = tokio::spawn(api::start_api(run_options, store.clone()));
    join_all(vec![submission_loop, verification_loop, server_loop]).await;
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
