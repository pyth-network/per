use {
    crate::{
        auction_old::get_express_relay_contract,
        config::{
            ChainId,
            Config,
            ConfigEvm,
            ConfigMap,
            SubwalletOptions,
        },
        state::ChainStoreEvm,
    },
    anyhow::{
        anyhow,
        Result,
    },
    ethers::{
        middleware::Middleware,
        prelude::LocalWallet,
        signers::Signer,
    },
};

pub async fn sync_subwallets(opts: SubwalletOptions) -> Result<()> {
    let config_map = ConfigMap::load(&opts.config.config).map_err(|err| {
        anyhow!(
            "Failed to load config from file({path}): {:?}",
            err,
            path = opts.config.config
        )
    })?;
    let wallet = opts.relayer_private_key.parse::<LocalWallet>()?;
    tracing::info!("Using wallet address: {:?}", wallet.address());
    for (chain_id, config) in config_map.chains.iter() {
        if let Config::Evm(chain_config) = config {
            if let Err(e) = sync_subwallets_for_chain(chain_id, chain_config, wallet.clone()).await
            {
                tracing::error!(
                    "Failed to sync subwallets for chain: {}. Error: {:?}",
                    chain_id,
                    e
                );
            }
        }
    }
    Ok(())
}

async fn sync_subwallets_for_chain(
    chain_id: &ChainId,
    chain_config: &ConfigEvm,
    wallet: LocalWallet,
) -> Result<()> {
    let provider = ChainStoreEvm::get_chain_provider(chain_id, chain_config)?;
    let id = provider.get_chainid().await?.as_u64();
    let express_relay_contract = get_express_relay_contract(
        chain_config.express_relay_contract,
        provider.clone(),
        wallet.clone(),
        chain_config.legacy_tx,
        id,
    );
    let current_relayer = express_relay_contract.get_relayer().call().await?;
    if current_relayer != wallet.address() {
        return Err(anyhow!(
            "Relayer address mismatch in the contract. Expected: {}, Got: {}",
            wallet.address(),
            current_relayer
        ));
    }
    let current_subwallets = express_relay_contract
        .get_relayer_subwallets()
        .call()
        .await?;
    let expected_subwallets = chain_config.clone().subwallets.unwrap_or_default();
    let all_subwallets = expected_subwallets.iter().chain(current_subwallets.iter());
    for subwallet in all_subwallets {
        if !current_subwallets.contains(subwallet) {
            express_relay_contract
                .add_relayer_subwallet(*subwallet)
                .send()
                .await?
                .await?;
            tracing::info!("Added subwallet: {:?}", subwallet);
        }
        if !expected_subwallets.contains(subwallet) {
            express_relay_contract
                .remove_relayer_subwallet(*subwallet)
                .send()
                .await?
                .await?;
            tracing::info!("Removed subwallet: {:?}", subwallet);
        }
    }
    Ok(())
}
