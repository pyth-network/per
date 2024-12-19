use {
    anyhow::{
        anyhow,
        Result,
    },
    clap::Parser,
    express_relay_client::{
        Client,
        ClientConfig,
    },
    express_relay_simple_searcher::SimpleSearcher,
};


#[derive(Parser, Clone, Debug)]
pub struct RunOptions {
    /// The http url of the express relay server.
    #[arg(long = "server-url")]
    #[arg(env = "SERVER_URL")]
    pub server_url: String,

    /// EVM private key in hex format.
    #[arg(long = "private-key-evm")]
    #[arg(env = "PRIVATE_KEY_EVM")]
    pub private_key_evm: Option<String>,

    /// SVM private key in base58 format.
    #[arg(long = "private-key-svm")]
    #[arg(env = "PRIVATE_KEY_SVM")]
    pub private_key_svm: Option<String>,

    /// Chain ids to subscribe to.
    #[arg(long = "chain-ids", required = true)]
    #[arg(env = "CHAIN_IDS")]
    pub chains: Vec<String>,

    /// The API key to use for authentication.
    #[arg(long = "api-key")]
    #[arg(env = "API_KEY")]
    pub api_key: Option<String>,

    /// The EVM config to override the default config.
    #[arg(long = "evm-config")]
    #[arg(env = "EVM_CONFIG")]
    pub evm_config: Option<String>,
}

// pub mod lib;

#[tokio::main]
async fn main() -> Result<()> {
    let args: RunOptions = RunOptions::parse();
    let client = Client::try_new(ClientConfig {
        http_url: args.server_url.clone(),
        api_key:  args.api_key.clone(),
    })
    .map_err(|e| {
        eprintln!("Failed to create client: {:?}", e);
        anyhow!("Failed to create client")
    })?;

    let simple_searcher = SimpleSearcher::try_new(
        client,
        args.chains,
        args.private_key_evm,
        args.private_key_svm,
    )
    .await?;
    simple_searcher.run().await
}
