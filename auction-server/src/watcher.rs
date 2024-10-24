use {
    crate::{
        auction::ChainStore,
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
    futures::StreamExt,
    solana_sdk::{
        commitment_config::CommitmentConfig,
        hash::Hash,
    },
    solana_transaction_status::TransactionDetails,
    std::{
        str::FromStr,
        sync::{
            atomic::Ordering,
            Arc,
        },
    },
};

pub async fn run_watcher_loop_svm(store: Arc<Store>, chain_id: String) -> Result<()> {
    let chain_store = store
        .chains_svm
        .get(&chain_id)
        .ok_or(anyhow!("Chain not found: {}", chain_id))?;

    let mut exit_check_interval = tokio::time::interval(EXIT_CHECK_INTERVAL);

    let ws_client = chain_store.get_ws_client().await?;
    let (mut stream, _) = ws_client
        .block_subscribe(
            solana_client::rpc_config::RpcBlockSubscribeFilter::All,
            Some(solana_client::rpc_config::RpcBlockSubscribeConfig {
                encoding:                          None,
                transaction_details:               Some(TransactionDetails::None), // we don't need transaction data
                show_rewards:                      Some(false),
                max_supported_transaction_version: None,
                commitment:                        Some(CommitmentConfig::finalized()),
            }),
        )
        .await?;

    while !SHOULD_EXIT.load(Ordering::Acquire) {
        tokio::select! {
            update = stream.next() => {
                if let Some(block_update) = update {
                    let blockhash = block_update.value.block.map(|b| b.blockhash).map(|b| Hash::from_str(&b)).transpose()?;
                    if let Some(blockhash) = blockhash {
                        store.broadcast_svm_chain_update(SvmChainUpdate {
                        chain_id: chain_id.clone(),
                        blockhash,
                        });

                        return Ok(());
                        }
                    else {
                        tracing::warn!(slot = block_update.value.slot, chain = chain_id, "Blockhash not found for slot");
                    }
                }
                else{
                    return Err(anyhow!("Watcher ended for chain: {}", chain_id));
                }
            }
            _ = exit_check_interval.tick() => {}
        }
    }
    Ok(())
}
