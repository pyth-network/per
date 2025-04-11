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

    /// SVM private key in base58 format.
    #[arg(long = "private-key-svm")]
    #[arg(env = "PRIVATE_KEY_SVM")]
    pub private_key_svm: Option<String>,

    /// Chain ids to subscribe to.
    #[arg(long = "chain-ids", required = true)]
    #[arg(env = "CHAIN_IDS")]
    pub chains: Vec<String>,

    /// The API key to use for auction server authentication.
    #[arg(long = "api-key")]
    #[arg(env = "API_KEY")]
    pub api_key: Option<String>,

    /// The SVM RPC URL.
    #[arg(long = "svm-rpc-url")]
    #[arg(env = "SVM_RPC_URL")]
    pub svm_rpc_url: Option<String>,
}

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

    let mut simple_searcher =
        SimpleSearcher::try_new(client, args.chains, args.private_key_svm, args.svm_rpc_url)
            .await?;
    simple_searcher.run().await
}
