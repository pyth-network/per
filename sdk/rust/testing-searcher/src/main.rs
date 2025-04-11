use {
    clap::Parser,
    express_relay_client::{
        solana_sdk::signature::{
            EncodableKey,
            Keypair,
        },
        Client,
        ClientConfig,
    },
    express_relay_simple_searcher::SimpleSearcher,
    std::env,
};


#[derive(Parser, Clone, Debug)]
pub struct RunOptions {
    /// The API key to use for auction server authentication.
    #[arg(long = "api-key")]
    #[arg(env = "API_KEY")]
    pub api_key: Option<String>,
}

#[tokio::main]
async fn main() {
    let args: RunOptions = RunOptions::parse();
    let svm_private_key_file =
        env::var("SVM_PRIVATE_KEY_FILE").expect("SVM_PRIVATE_KEY_FILE is not set");
    let svm_rpc_url = "http://127.0.0.1:8899";
    let server_url = "http://127.0.0.1:9000";

    let svm_private_key = Keypair::read_from_file(svm_private_key_file.clone())
        .expect("Failed to read SVM private key");

    let client = Client::try_new(ClientConfig {
        http_url: server_url.to_string(),
        api_key:  args.api_key,
    })
    .expect("Failed to create client");

    let mut searcher = SimpleSearcher::try_new(
        client,
        vec!["local-solana".to_string()],
        Some(svm_private_key.to_base58_string()),
        Some(svm_rpc_url.to_string()),
    )
    .await
    .expect("Failed to create searcher");
    searcher.run().await.expect("Failed to run searcher");
}
