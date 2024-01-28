use {
    crate::config::{
        DeployOptions,
        RunOptions,
    },
    anyhow::{
        anyhow,
        Context,
        Result,
    },
    base64::prelude::*,
    ethers::{
        abi::Address,
        contract::{
            abigen,
            ContractError,
        },
        core::utils::hex::FromHex,
        middleware::SignerMiddleware,
        providers::{
            Http,
            Middleware,
            Provider,
        },
        signers::{
            LocalWallet,
            Signer,
        },
        types::{
            Bytes,
            U256,
        },
    },
    rand::{
        random,
        seq::SliceRandom,
    },
    serde_json::Value,
    std::sync::Arc,
    url::Url,
};

abigen!(
    TokenVault,
    "../per_multicall/out/TokenVault.sol/TokenVault.json"
);

abigen!(ERC20, "../per_multicall/out/MyToken.sol/MyToken.json");
abigen!(IPyth, "../per_multicall/out/IPyth.sol/IPyth.json");

pub type SignableTokenVaultContract = TokenVault<SignerMiddleware<Provider<Http>, LocalWallet>>;

#[derive(Clone)]
struct PythUpdate {
    price: U256,
    vaa:   Bytes,
}

#[derive(Clone)]
struct TokenInfo {
    symbol:   String,
    price_id: String,
    address:  Address,
    contract: ERC20<SignerMiddleware<Provider<Http>, LocalWallet>>,
}

async fn get_token_info(
    token: Address,
    client: Arc<SignerMiddleware<Provider<Http>, LocalWallet>>,
) -> Result<TokenInfo> {
    let contract = ERC20::new(token, client.clone());
    let symbol = contract.symbol().await?;
    let price_id = contract.name().await?;
    Ok(TokenInfo {
        symbol,
        price_id,
        address: token,
        contract,
    })
}

fn parse_update(update: Value) -> Result<PythUpdate> {
    let price_component = update["price"].clone();
    let price = U256::from_dec_str(price_component["price"].as_str().unwrap()).unwrap();
    let expo = price_component["expo"].as_i64().unwrap() + 18; // use 18 as max exponent
    tracing::info!("Price: {}, Exponent: {}", price, expo);
    let multiple = U256::exp10(expo.try_into().map_err(|_| anyhow!("Invalid exponent"))?);

    Ok(PythUpdate {
        price: price * multiple,
        vaa:   Bytes::from(
            BASE64_STANDARD
                .decode(update["vaa"].as_str().unwrap())
                .unwrap(),
        ),
    })
}

async fn get_latest_updates(feed_ids: Vec<String>) -> Result<Vec<PythUpdate>> {
    let url = Url::parse_with_params(
        "https://hermes.pyth.network/api/latest_price_feeds?verbose=true&binary=true",
        feed_ids
            .iter()
            .map(|id| ("ids[]", id.to_string()))
            .collect::<Vec<_>>()
            .as_slice(),
    )?;
    let response = reqwest::get(url).await?;
    let updates = response.json::<serde_json::Value>().await?;
    (updates)
        .as_array()
        .context(format!("Invalid response: {:?}", updates))?
        .into_iter()
        .map(|update| parse_update(update.clone()))
        .collect()
}

pub async fn run_simulator(options: RunOptions) -> Result<()> {
    let wallet = options
        .private_key
        .parse::<LocalWallet>()
        .map_err(|e| anyhow!("Can not parse private key: {}", e))?;
    tracing::info!("Using wallet address: {}", wallet.address().to_string());
    let provider = Provider::<Http>::try_from(options.rpc_addr.as_str()).map_err(|err| {
        anyhow!(
            "Failed to connect to {rpc_addr}: {:?}",
            err,
            rpc_addr = options.rpc_addr
        )
    })?;
    let chain_id = provider.get_chainid().await?;
    tracing::info!("Connected to chain: {}", chain_id);

    // check the balance of the wallet
    let wallet_address = wallet.address();
    let balance = provider.get_balance(wallet_address, None).await?;
    tracing::info!("Wallet balance: {}", balance);

    let client = Arc::new(SignerMiddleware::new(
        provider,
        wallet.with_chain_id(chain_id.as_u64()),
    ));

    let sample: [&Address; 2] = options
        .tokens
        .choose_multiple(&mut rand::thread_rng(), 2)
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| anyhow!("Unable to sample 2 tokens as colateral and debt"))?;

    let [collateral, debt] = sample;
    let collateral_info = get_token_info(*collateral, client.clone()).await?;
    let debt_info = get_token_info(*debt, client.clone()).await?;

    tracing::info!(
        "Collateral Symbol: {} price id: {}",
        collateral_info.symbol,
        collateral_info.price_id
    );
    tracing::info!(
        "Debt Symbol: {} price id: {}",
        debt_info.symbol,
        debt_info.price_id
    );

    // get the latest pyth updates
    let updates = get_latest_updates(vec![
        collateral_info.price_id.clone(),
        debt_info.price_id.clone(),
    ])
    .await?;
    let collateral_update = updates[0].clone();
    let debt_update = updates[1].clone();

    // usd value random between 100 and 1000 dollars
    let precision = U256::exp10(18);
    let collateral_value_usd: U256 = precision * U256::from(random::<u64>() % 900 + 100);
    tracing::info!("Collateral value usd: {}", collateral_value_usd);

    tracing::info!("Collateral price: {}", collateral_update.price);
    tracing::info!("Debt price: {}", collateral_update.price);

    let amount_collateral: U256 =
        collateral_value_usd * precision * 111 / 100 / collateral_update.price; // 10% more safety margin
    let amount_debt = collateral_value_usd * precision / debt_update.price;

    let min_health_ratio = U256::exp10(18) * 110 / 100;
    let min_permission_less_health_ratio = U256::exp10(18) * 105 / 100;

    let token_id_collateral: [u8; 32] = <[u8; 32]>::from_hex(collateral_info.price_id).unwrap();
    let token_id_debt: [u8; 32] = <[u8; 32]>::from_hex(debt_info.price_id).unwrap();
    let update_data = vec![collateral_update.vaa, debt_update.vaa];
    // let update_data = vec![];

    collateral_info
        .contract
        .mint(wallet_address, amount_collateral)
        .send()
        .await?;
    collateral_info
        .contract
        .approve(options.contract, amount_collateral)
        .send()
        .await?;

    tracing::info!("Calling create_vault");
    tracing::info!("Amount collateral: {}", amount_collateral);
    tracing::info!("Amount debt: {}", amount_debt);

    let contract = SignableTokenVaultContract::new(options.contract, client.clone());
    let tx = contract
        .create_vault(
            collateral_info.address,
            debt_info.address,
            amount_collateral,
            amount_debt,
            min_health_ratio,
            min_permission_less_health_ratio,
            token_id_collateral,
            token_id_debt,
            update_data.clone(),
        )
        .value(update_data.len());
    let result: Result<_, ContractError<_>> = tx.send().await;
    match result {
        Ok(_) => {
            tracing::info!("Vault created");
        }
        Err(e) => {
            let decoded = e.decode_contract_revert::<token_vault::TokenVaultErrors>();
            tracing::info!("Error creating vault: {:?}", decoded);
        }
    }

    Ok(())
}

pub async fn deploy_contract(options: DeployOptions) -> Result<()> {
    let wallet = options
        .private_key
        .parse::<LocalWallet>()
        .map_err(|e| anyhow!("Can not parse private key: {}", e))?;
    tracing::info!("Using wallet address: {}", wallet.address().to_string());
    let provider = Provider::<Http>::try_from(options.rpc_addr.as_str()).map_err(|err| {
        anyhow!(
            "Failed to connect to {rpc_addr}: {:?}",
            err,
            rpc_addr = options.rpc_addr
        )
    })?;
    let chain_id = provider.get_chainid().await?;
    tracing::info!("Connected to chain id: {}", chain_id);

    let client = Arc::new(SignerMiddleware::new(
        provider,
        wallet.with_chain_id(chain_id.as_u64()),
    ));
    let contract = SignableTokenVaultContract::deploy(
        client,
        (options.per_contract, options.oracle_contract),
    )?
    .send()
    .await?;
    tracing::info!("{}", contract.address().to_string());
    Ok(())
}