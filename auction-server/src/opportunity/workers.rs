use {
    super::service::{
        ChainTypeEvm,
        Service,
    },
    crate::server::{
        EXIT_CHECK_INTERVAL,
        SHOULD_EXIT,
    },
    std::{
        sync::{
            atomic::Ordering,
            Arc,
        },
        time::Duration,
    },
};

pub async fn run_verification_loop(service: Arc<Service<ChainTypeEvm>>) -> anyhow::Result<()> {
    tracing::info!("Starting opportunity verifier...");
    let mut exit_check_interval = tokio::time::interval(EXIT_CHECK_INTERVAL);

    // this should be replaced by a subscription to the chain and trigger on new blocks
    let mut submission_interval = tokio::time::interval(Duration::from_secs(5));
    while !SHOULD_EXIT.load(Ordering::Acquire) {
        tokio::select! {
            _ = submission_interval.tick() => {
                service.remove_invalid_or_expired_opportunities().await;
            }
            _ = exit_check_interval.tick() => {
            }
        }
    }
    tracing::info!("Shutting down opportunity verifier...");
    Ok(())
}
