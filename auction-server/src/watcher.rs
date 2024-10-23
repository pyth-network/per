use std::{str::FromStr, sync::atomic::Ordering};

use crate::{auction::ChainStore, server::{EXIT_CHECK_INTERVAL, SHOULD_EXIT}, state::ChainStoreSvm};
use anyhow::Result;
use futures::StreamExt;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_transaction_status::TransactionDetails;
use solana_sdk::hash::Hash;

pub async fn run_svm_watcher_loop(chain_store: &ChainStoreSvm) -> Result<()> {
    let mut exit_check_interval = tokio::time::interval(EXIT_CHECK_INTERVAL);

    let ws_client = chain_store.get_ws_client().await?;
    let (mut stream, _) = ws_client.block_subscribe(
        solana_client::rpc_config::RpcBlockSubscribeFilter::All,
        Some(solana_client::rpc_config::RpcBlockSubscribeConfig {
            encoding:                          None,
            transaction_details:               Some(TransactionDetails::None),
            show_rewards:                      Some(false),
            max_supported_transaction_version: None,
            commitment:                        Some(CommitmentConfig::finalized()),
        }),
    ).await?;

    while !SHOULD_EXIT.load(Ordering::Acquire) {
        tokio::select! {
            block_update = stream.next() => {
                    let blockhash = block_update
            .and_then(|t| t.value.block.map(|b| b.blockhash))
                        .and_then(|b| Hash::from_str(&b).ok());
                tracing::info!("New blockhash received: {:?}", blockhash);
                let mut recent_block_hash = 
                chain_store.recent_blockhash.write().await;
                *recent_block_hash =blockhash;
            }

            _ = exit_check_interval.tick() => {}
        }
    }
    Ok(())
}
