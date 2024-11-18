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
    serde::{
        Deserialize,
        Serialize,
    },
    solana_sdk::{
        clock::Slot,
        commitment_config::CommitmentConfig,
    },
    std::{
        sync::{
            atomic::Ordering,
            Arc,
        },
        time::Duration,
    },
};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RpcPrioritizationFee {
    pub slot:               Slot,
    pub prioritization_fee: u64,
}

pub const GET_LATEST_BLOCKHASH_INTERVAL: Duration = Duration::from_secs(5);

pub async fn run_watcher_loop_svm(store: Arc<Store>, chain_id: String) -> Result<()> {
    let chain_store = store
        .chains_svm
        .get(&chain_id)
        .ok_or(anyhow!("Chain not found: {}", chain_id))?;

    while !SHOULD_EXIT.load(Ordering::Acquire) {
        match (
            chain_store
                .client
                .get_latest_blockhash_with_commitment(CommitmentConfig::finalized())
                .await,
            chain_store.get_and_store_recent_prioritization_fee().await,
        ) {
            (Ok(result), Ok(fee)) => {
                store.broadcast_svm_chain_update(SvmChainUpdate {
                    chain_id:                  chain_id.clone(),
                    blockhash:                 result.0,
                    latest_prioritization_fee: fee,
                });
            }
            (Err(e), _) => {
                return Err(anyhow!(
                    "Polling blockhash failed for chain {} with error: {}",
                    chain_id,
                    e
                ));
            }
            (_, Err(e)) => {
                return Err(anyhow!(
                    "Polling prioritization fees failed for chain {} with error: {}",
                    chain_id,
                    e
                ));
            }
        }

        tokio::time::sleep(GET_LATEST_BLOCKHASH_INTERVAL).await;
    }
    Ok(())
}
