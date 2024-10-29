use {
    crate::{
        server::{
            EXIT_CHECK_INTERVAL,
            SHOULD_EXIT,
        },
        state::{
            Store,
            SvmChainUpdate,
        },
    },
    anyhow::{
        anyhow,
        Result,
    },
    std::{
        sync::{
            atomic::Ordering,
            Arc,
        },
        time::Duration,
    },
};

pub const GET_LATEST_BLOCKHASH_INTERVAL: u64 = 1000;

pub async fn run_watcher_loop_svm(store: Arc<Store>, chain_id: String) -> Result<()> {
    let chain_store = store
        .chains_svm
        .get(&chain_id)
        .ok_or(anyhow!("Chain not found: {}", chain_id))?;

    let mut exit_check_interval = tokio::time::interval(EXIT_CHECK_INTERVAL);

    while !SHOULD_EXIT.load(Ordering::Acquire) {
        tokio::time::sleep(Duration::from_millis(GET_LATEST_BLOCKHASH_INTERVAL)).await;
        tokio::select! {
            response = chain_store.client.get_latest_blockhash() => {
                if let Ok(blockhash) = response {
                    store.broadcast_svm_chain_update(SvmChainUpdate {
                        chain_id: chain_id.clone(),
                        blockhash,
                    })
                } else {
                    return Err(anyhow!("Polling blockhash failed for chain: {}", chain_id));
                }
            }
            _ = exit_check_interval.tick() => {}
        }
    }
    Ok(())
}
