use {
    crate::{
        server::SHOULD_EXIT,
        state::{
            Store,
            SvmChainUpdate,
        },
    },
    anyhow::{
        anyhow,
        Result,
    },
    solana_sdk::commitment_config::CommitmentConfig,
    std::{
        sync::{
            atomic::Ordering,
            Arc,
        },
        time::Duration,
    },
};

pub const GET_LATEST_BLOCKHASH_INTERVAL: Duration = Duration::from_secs(1);
pub const GET_LATEST_BLOCKHASH_TIMEOUT: Duration = Duration::from_secs(2);

pub async fn run_watcher_loop_svm(store: Arc<Store>, chain_id: String) -> Result<()> {
    let chain_store = store
        .chains_svm
        .get(&chain_id)
        .ok_or(anyhow!("Chain not found: {}", chain_id))?;

    while !SHOULD_EXIT.load(Ordering::Acquire) {
        tokio::select! {
            response = chain_store.client.get_latest_blockhash_with_commitment(CommitmentConfig::finalized()) => {
                if let Ok(result) = response {
                    store.broadcast_svm_chain_update(SvmChainUpdate {
                        chain_id: chain_id.clone(),
                        blockhash: result.0,
                    })
                } else {
                    tracing::info!("Polling blockhash failed for chain: {}", chain_id);
                    return Err(anyhow!("Polling blockhash failed for chain: {}", chain_id));
                }
            }
            _ = tokio::time::sleep(GET_LATEST_BLOCKHASH_TIMEOUT) => {
                return Err(anyhow!("Polling blockhash timed out for chain: {}", chain_id));
            }
        }
        tokio::time::sleep(GET_LATEST_BLOCKHASH_INTERVAL).await;
    }
    Ok(())
}
