use {
    anyhow::Result,
    clap::{crate_authors, crate_description, crate_name, crate_version, Args, Parser},
    ethers::abi::Address,
    std::{collections::HashMap, fs},
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
}

#[derive(Args, Clone, Debug)]
pub struct RunOptions {
    /// Server Options
    #[command(flatten)]
    pub server: server::Options,

    #[command(flatten)]
    pub config: ConfigOptions,

    /// A 20-byte (40 char) hex encoded Ethereum private key which is used for relaying the bids.
    #[arg(long = "relayer-private-key")]
    #[arg(env = "RELAYER_PRIVATE_KEY")]
    pub relayer_private_key: String,
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
pub struct Config {
    pub chains: HashMap<ChainId, EthereumConfig>,
}

impl Config {
    pub fn load(path: &str) -> Result<Config> {
        // Open and read the YAML file
        // TODO: the default serde deserialization doesn't enforce unique keys
        let yaml_content = fs::read_to_string(path)?;
        let config: Config = serde_yaml::from_str(&yaml_content)?;
        Ok(config)
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct EthereumConfig {
    /// URL of a Geth RPC endpoint to use for interacting with the blockchain.
    pub geth_rpc_addr: String,

    /// Polling interval for event filters and pending transactions in seconds.
    pub poll_interval: u64,

    /// Address of the express relay contract to interact with.
    pub express_relay_contract: Address,

    /// Address of the opportunity adapter contract to interact with.
    pub opportunity_adapter_contract: Address,

    /// Use the legacy transaction format (for networks without EIP 1559)
    #[serde(default)]
    pub legacy_tx: bool,
}
