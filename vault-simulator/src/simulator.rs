use {
    crate::config::{
        DeployOptions,
        SearcherOptions,
        SimulatorOptions,
    },
    anyhow::{
        anyhow,
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
        middleware::{
            gas_oracle::GasOracleMiddleware,
            SignerMiddleware,
        },
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
            TransactionRequest,
            U256,
        },
    },
    gas_oracle::EthProviderOracle,
    rand::{
        random,
        seq::SliceRandom,
    },
    serde_json::Value,
    std::{
        sync::Arc,
        time::Duration,
    },
    url::Url,
};

abigen!(
    TokenVault,
    "../contracts/evm/out/TokenVault.sol/TokenVault.json"
);

abigen!(ERC20, "../contracts/evm/out/MyToken.sol/MyToken.json");
abigen!(WETH9, "../contracts/evm/out/WETH9.sol/WETH9.json");
abigen!(IPyth, "../contracts/evm/out/IPyth.sol/IPyth.json");

type SimulatorProvider = GasOracleMiddleware<
    SignerMiddleware<Provider<Http>, LocalWallet>,
    EthProviderOracle<Provider<Http>>,
>;
pub type SignableTokenVaultContract = TokenVault<SimulatorProvider>;
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
    contract: ERC20<SimulatorProvider>,
}

async fn get_token_info(token: Address, client: Arc<SimulatorProvider>) -> Result<TokenInfo> {
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

async fn setup_client(private_key: String, rpc_address: Url) -> Result<Arc<SimulatorProvider>> {
    let wallet = private_key
        .parse::<LocalWallet>()
        .map_err(|e| anyhow!("Can not parse private key: {}", e))?;
    tracing::info!("Using wallet address: {}", wallet.address().to_string());
    let mut provider = Provider::<Http>::try_from(rpc_address.as_str()).map_err(|err| {
        anyhow!(
            "Failed to connect to {rpc_addr}: {:?}",
            err,
            rpc_addr = rpc_address.as_str()
        )
    })?;
    provider.set_interval(Duration::from_secs(1));
    let chain_id = provider.get_chainid().await?;

    tracing::info!("Connected to chain: {}", chain_id);
    let client = Arc::new(GasOracleMiddleware::new(
        SignerMiddleware::new(provider.clone(), wallet.with_chain_id(chain_id.as_u64())),
        EthProviderOracle::new(provider),
    ));
    Ok(client)
}

async fn get_latest_updates(price_endpoint: Url, feed_ids: Vec<String>) -> Result<Vec<PythUpdate>> {
    let base_url = price_endpoint.join("/api/latest_price_feeds?verbose=true&binary=true")?;
    let url = Url::parse_with_params(
        base_url.as_str(),
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
        .ok_or(anyhow!("Invalid response: {:?}", updates))?
        .iter()
        .map(|update| parse_update(update.clone()))
        .collect()
}

pub async fn run_simulator(simulator_options: SimulatorOptions) -> Result<()> {
    let options = simulator_options.run_options;
    let client = setup_client(options.private_key, options.rpc_addr).await?;
    let wallet_address = client.inner().signer().address();
    let balance = client.get_balance(wallet_address, None).await?;
    tracing::info!("Wallet balance: {}", balance);

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
    let updates = get_latest_updates(
        simulator_options.price_endpoint,
        vec![collateral_info.price_id.clone(), debt_info.price_id.clone()],
    )
    .await?;
    let collateral_update = updates[0].clone();
    let debt_update = updates[1].clone();

    let precision = U256::exp10(18);
    // usd value random between 100 and 1000 dollars
    let collateral_value_usd: U256 = precision * U256::from(random::<u64>() % 900 + 100);
    tracing::info!("Collateral value usd: {}", collateral_value_usd);
    tracing::info!("Collateral price: {}", collateral_update.price);
    tracing::info!("Debt price: {}", debt_update.price);

    let contract =
        SignableTokenVaultContract::new(simulator_options.vault_contract, client.clone());

    let min_health_numerator: U256 = U256::from(1100000);
    let min_health_permissionless_numerator: U256 = U256::from(1050000);
    let min_health_denominator: U256 = U256::from(1000000);
    let min_health_ratio = U256::exp10(18) * min_health_numerator / min_health_denominator;
    let min_permissionless_health_ratio =
        U256::exp10(18) * min_health_permissionless_numerator / min_health_denominator;

    let collat_ratio: U256;
    let allow_undercollateralized: bool = contract.get_allow_undercollateralized().call().await?;
    if allow_undercollateralized {
        // Less than min_health_ratio, greater than permissionless ratio, to create the vault undercollateralized
        collat_ratio = (min_health_numerator + min_health_permissionless_numerator) / 2;
    } else {
        // Slightly more than min_health_ratio to create the vault overcollateralized
        collat_ratio = min_health_numerator * 10_001 / 10_000;
    }

    let amount_collateral: U256 = collateral_value_usd * precision * collat_ratio
        / min_health_denominator
        / collateral_update.price;
    let amount_debt = collateral_value_usd * precision / debt_update.price;

    let token_id_collateral: [u8; 32] = <[u8; 32]>::from_hex(collateral_info.price_id).unwrap();
    let token_id_debt: [u8; 32] = <[u8; 32]>::from_hex(debt_info.price_id).unwrap();
    let update_data = vec![collateral_update.vaa, debt_update.vaa];

    collateral_info
        .contract
        .mint(wallet_address, amount_collateral)
        .send()
        .await?
        .await?;
    collateral_info
        .contract
        .approve(simulator_options.vault_contract, amount_collateral)
        .send()
        .await?
        .await?;

    tracing::info!("Amount collateral: {}", amount_collateral);
    tracing::info!("Amount debt: {}", amount_debt);

    let tx = contract
        .create_vault(
            collateral_info.address,
            debt_info.address,
            amount_collateral,
            amount_debt,
            min_health_ratio,
            min_permissionless_health_ratio,
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
    let client = setup_client(options.private_key, options.rpc_addr).await?;
    let admin = client.inner().signer().address();
    let allow_undercollateralized = true;
    let contract = SignableTokenVaultContract::deploy(
        client,
        (
            admin,
            options.relay_contract,
            options.oracle_contract,
            allow_undercollateralized,
        ),
    )?
    .send()
    .await?;
    tracing::info!("{:?}", contract.address());
    Ok(())
}

pub async fn create_searcher(searcher_options: SearcherOptions) -> Result<()> {
    let options = searcher_options.run_options;
    let funder_client = setup_client(options.private_key, options.rpc_addr.clone()).await?;
    let client = setup_client(searcher_options.searcher_private_key, options.rpc_addr).await?;
    let wallet_address = client.inner().signer().address();
    let tx = TransactionRequest::new()
        .to(wallet_address)
        .value(U256::exp10(16))
        .from(funder_client.inner().signer().address());
    funder_client.send_transaction(tx, None).await?.await?;
    tracing::info!("0.01 ETH sent to searcher wallet");
    for token in options.tokens.iter() {
        let token_contract = ERC20::new(*token, client.clone());
        token_contract
            .approve(searcher_options.adapter_contract, U256::MAX)
            .send()
            .await?
            .await?;
        token_contract
            .mint(wallet_address, U256::exp10(36))
            .send()
            .await?
            .await?;
        tracing::info!(
            "Token {} minted and approved to use by liquidation adapter",
            token.to_string()
        );
    }

    let weth_contract = WETH9::new(options.weth, client.clone());
    weth_contract
        .deposit()
        .value(U256::exp10(14))
        .send()
        .await?
        .await?;
    weth_contract
        .approve(searcher_options.adapter_contract, U256::MAX)
        .send()
        .await?
        .await?;
    let balance = weth_contract.balance_of(wallet_address).await?;
    tracing::info!(
        ".0001 ETH deposited into WETH and approved to use by liquidation adapter, current balance: {} wei",
        balance
    );
    Ok(())
}
