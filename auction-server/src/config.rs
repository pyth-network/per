use {
    anyhow::Result,
    clap::{
        crate_authors,
        crate_description,
        crate_name,
        crate_version,
        Args,
        Parser,
    },
    ethers::abi::Address,
    serde_with::{
        serde_as,
        DisplayFromStr,
    },
    solana_sdk::pubkey::Pubkey,
    std::{
        collections::HashMap,
        fs,
    },
};

mod server;

// `Options` is a structup definition to provide clean command-line args for Hermes.
#[derive(Parser, Debug)]
#[command(name = crate_name!())]
#[command(author = crate_authors!())]
#[command(about = crate_description!())]
#[command(version = crate_version!())]
#[allow(clippy::large_enum_variant)]
pub enum Options {
    /// Run the auction server service.
    Run(RunOptions),
    /// Sync the relayer subwallets
    SyncSubwallets(SubwalletOptions),
}

#[derive(Args, Clone, Debug)]
pub struct SubwalletOptions {
    #[command(flatten)]
    pub config: ConfigOptions,

    /// A 20-byte (40 char) hex encoded Ethereum private key which is used for relaying the bids.
    #[arg(long = "relayer-private-key")]
    #[arg(env = "RELAYER_PRIVATE_KEY")]
    pub relayer_private_key: String,
}

#[derive(Args, Clone, Debug)]
pub struct RunOptions {
    /// Server Options
    #[command(flatten)]
    pub server: server::Options,

    #[command(flatten)]
    pub config: ConfigOptions,

    /// A 20-byte (40 char) hex encoded Ethereum private key for one of the subwallets
    /// which can be used for relaying the bids.
    #[arg(long = "subwallet-private-key")]
    #[arg(env = "SUBWALLET_PRIVATE_KEY")]
    pub subwallet_private_key: String,

    /// SVM relayer private key in base58 format
    #[arg(long = "private-key-svm")]
    #[arg(env = "PRIVATE_KEY_SVM")]
    pub private_key_svm: Option<String>,

    #[arg(long = "secret-key")]
    #[arg(env = "SECRET_KEY")]
    pub secret_key: String,
}

#[derive(Args, Clone, Debug)]
#[command(next_help_heading = "Config Options")]
#[group(id = "Config")]
pub struct ConfigOptions {
    /// Path to a configuration file containing the list of supported blockchains
    #[arg(long = "config")]
    #[arg(env = "PER_CONFIG")]
    #[arg(default_value = "config.yaml")]
    pub config: String,
}

pub type ChainId = String;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ConfigMap {
    pub chains: HashMap<ChainId, Config>,
}

impl ConfigMap {
    pub fn load(path: &str) -> Result<ConfigMap> {
        // Open and read the YAML file
        // TODO: the default serde deserialization doesn't enforce unique keys
        let yaml_content = fs::read_to_string(path)?;
        let config: ConfigMap = serde_yaml::from_str(&yaml_content)?;
        Ok(config)
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(untagged)] // Remove tags to avoid key-value wrapping
pub enum Config {
    Evm(ConfigEvm),
    Svm(ConfigSvm),
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ConfigEvm {
    /// URL of a Geth RPC endpoint to use for interacting with the blockchain.
    pub geth_rpc_addr: String,

    /// URL of a Geth WS endpoint to use for interacting with the blockchain.
    pub geth_ws_addr: String,

    /// Timeout for RPC requests in seconds.
    pub rpc_timeout: u64,

    /// Polling interval for event filters and pending transactions in seconds.
    pub poll_interval: u64,

    /// Address of the express relay contract to interact with.
    pub express_relay_contract: Address,

    /// Address of the opportunity adapter factory contract to interact with.
    pub adapter_factory_contract: Address,

    /// Subwallets available for relaying bids. Only used in the subwallet sync command.
    pub subwallets: Option<Vec<Address>>,

    /// Use the legacy transaction format (for networks without EIP 1559)
    #[serde(default)]
    pub legacy_tx: bool,
}

fn default_rpc_timeout_svm() -> u64 {
    2
}

#[serde_as]
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ConfigSvm {
    /// Id of the express relay program.
    #[serde_as(as = "DisplayFromStr")]
    pub express_relay_program_id: Pubkey,
    /// RPC endpoint to use for interacting with the blockchain.
    pub rpc_addr:                 String,
    /// WS endpoint to use for interacting with the blockchain.
    pub ws_addr:                  String,
    /// Timeout for RPC requests in seconds.
    #[serde(default = "default_rpc_timeout_svm")]
    pub rpc_timeout:              u64,
}
