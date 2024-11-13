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

pub const GET_LATEST_BLOCKHASH_INTERVAL: Duration = Duration::from_secs(5);

pub async fn run_watcher_loop_svm(store: Arc<Store>, chain_id: String) -> Result<()> {
    let chain_store = store
        .chains_svm
        .get(&chain_id)
        .ok_or(anyhow!("Chain not found: {}", chain_id))?;

    while !SHOULD_EXIT.load(Ordering::Acquire) {
        let response = chain_store
            .client
            .get_latest_blockhash_with_commitment(CommitmentConfig::finalized())
            .await;

        match response {
            Ok(result) => store.broadcast_svm_chain_update(SvmChainUpdate {
                chain_id:       chain_id.clone(),
                blockhash:      result.0,
                compute_budget: chain_store.get_latest_compute_budget().await.unwrap_or(0),
            }),
            Err(e) => {
                return Err(anyhow!(
                    "Polling blockhash failed for chain {} with error: {}",
                    chain_id,
                    e
                ));
            }
        }

        tokio::time::sleep(GET_LATEST_BLOCKHASH_INTERVAL).await;
    }
    Ok(())
}
