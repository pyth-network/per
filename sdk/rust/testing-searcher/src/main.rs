use {
    express_relay_client::{
        ethers::utils::hex,
        evm::Config,
        Client,
        ClientConfig,
    },
    express_relay_simple_searcher::SimpleSearcher,
    std::{
        collections::HashMap,
        env,
    },
};

#[tokio::main]
async fn main() {
    let weth = env::var("WETH").expect("WETH is not set");
    let searcher_sk = env::var("SEARCHER_SK").expect("SEARCHER_SK is not set");
    let adapter_factory = env::var("ADAPTER_FACTORY").expect("ADAPTER_FACTORY is not set");
    let adapter_bytecode_hash =
        env::var("ADAPTER_BYTECODE_HASH").expect("ADAPTER_BYTECODE_HASH is not set");
    let permit2 = env::var("PERMIT2").expect("PERMIT2 is not set");
    let chain_id_num = env::var("CHAIN_ID_NUM").expect("CHAIN_ID_NUM is not set");
    let server_url = "http://127.0.0.1:9000";

    let config = Config {
        weth:                     weth.parse().unwrap(),
        permit2:                  permit2.parse().unwrap(),
        adapter_factory_contract: adapter_factory.parse().unwrap(),
        adapter_bytecode_hash:    hex::decode(adapter_bytecode_hash)
            .unwrap()
            .try_into()
            .unwrap(),
        chain_id_num:             chain_id_num.parse().unwrap(),
    };
    let chain_id = "development".to_string();
    let mut config_map: HashMap<String, Config> = HashMap::new();
    config_map.insert(chain_id.clone(), config);

    let client = Client::try_new_with_evm_config(
        ClientConfig {
            http_url: server_url.to_string(),
            api_key:  None,
        },
        config_map.clone(),
    )
    .expect("Failed to create client");

    let searcher = SimpleSearcher::try_new(client, vec![chain_id.clone()], Some(searcher_sk), None)
        .await
        .expect("Failed to create searcher");
    searcher.run().await.expect("Failed to run searcher");
}
