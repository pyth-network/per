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
    solana_client::{
        client_error::ClientError,
        nonblocking::rpc_client::RpcClient,
    },
    solana_sdk::{
        clock::{
            Slot,
            DEFAULT_MS_PER_SLOT,
        },
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
pub const RECENT_FEES_SLOT_WINDOW: usize =
    GET_LATEST_BLOCKHASH_INTERVAL.as_millis() as usize / DEFAULT_MS_PER_SLOT as usize;

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
            get_median_prioritization_fee(
                &chain_store.client,
                chain_store.config.prioritization_fee_percentile,
            )
            .await,
        ) {
            (Ok(result), Ok(median_fee)) => {
                store.broadcast_svm_chain_update(SvmChainUpdate {
                    chain_id:  chain_id.clone(),
                    blockhash: result.0,
                });
                tracing::info!(
                    "Median prioritization fee for chain {} is {}",
                    chain_id,
                    median_fee
                );
                chain_store.set_median_prioritization_fee(median_fee).await;
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

pub async fn get_median_prioritization_fee(
    client: &RpcClient,
    percentile: Option<u64>,
) -> Result<u64, ClientError> {
    let accounts: Vec<String> = vec![];
    let mut args: Vec<serde_json::Value> = vec![serde_json::to_value(accounts)?];

    if let Some(percentile) = percentile {
        args.push(serde_json::json!({ "percentile": percentile }));
    }

    fn median(values: &mut [u64]) -> u64 {
        let mid = (values.len() + 1) / 2;
        *values.select_nth_unstable(mid).1
    }

    client
        .send(
            solana_client::rpc_request::RpcRequest::GetRecentPrioritizationFees,
            serde_json::Value::from(args),
        )
        .await
        .map(|mut values: Vec<RpcPrioritizationFee>| {
            values.sort_by(|a, b| b.slot.cmp(&a.slot));
            median(
                &mut values
                    .iter()
                    .take(RECENT_FEES_SLOT_WINDOW)
                    .map(|fee| fee.prioritization_fee)
                    .collect::<Vec<u64>>(),
            )
        })
}
