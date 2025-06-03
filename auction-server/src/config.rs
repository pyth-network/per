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
    serde_with::{
        serde_as,
        DisplayFromStr,
    },
    server::ClickhouseConfig,
    solana_sdk::pubkey::Pubkey,
    std::{
        collections::HashMap,
        fs,
        time::Duration,
    },
};

pub mod server;

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
    /// Run db migrations and exit.
    Migrate(MigrateOptions),
    /// Run clickhouse migrations and exit.
    MigrateClickhouse(ClickhouseConfig),
}

#[derive(Args, Clone, Debug)]
pub struct MigrateOptions {
    /// database url to run the migrations for.
    #[arg(long = "database-url")]
    #[arg(env = "DATABASE_URL")]
    pub database_url: String,
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
    /// The options for server.
    #[command(flatten)]
    pub server: server::Options,

    #[command(flatten)]
    pub config: ConfigOptions,

    /// SVM relayer private key in base58 format.
    #[arg(long = "private-key-svm")]
    #[arg(env = "PRIVATE_KEY_SVM")]
    pub private_key_svm: Option<String>,

    #[arg(long = "secret-key")]
    #[arg(env = "SECRET_KEY")]
    pub secret_key: String,

    #[command(flatten)]
    pub delete_pg_rows: DeletePgRowsOptions,
}

#[derive(Args, Clone, Debug)]
#[command(next_help_heading = "Delete PG Rows Options")]
#[group(id = "DeletePgRows")]
pub struct DeletePgRowsOptions {
    /// How often to delete rows from the database.
    #[arg(long = "delete-interval-seconds")]
    #[arg(env = "DELETE_INTERVAL_SECONDS")]
    pub delete_interval_secs: Option<u64>,

    /// The threshold staleness for whether a row should be deleted.
    #[arg(long = "delete-threshold-seconds")]
    #[arg(env = "DELETE_THRESHOLD_SECONDS")]
    pub delete_threshold_secs: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct DeletePgRowsFlags {
    pub delete_interval_secs:  u64,
    pub delete_threshold_secs: u64,
}

impl DeletePgRowsOptions {
    pub fn into_option(self) -> Option<DeletePgRowsFlags> {
        match (self.delete_interval_secs, self.delete_threshold_secs) {
            (Some(interval), Some(threshold)) => Some(DeletePgRowsFlags {
                delete_interval_secs:  interval,
                delete_threshold_secs: threshold,
            }),
            (None, None) => None,
            _ => {
                tracing::warn!("Both --delete-interval-seconds and --delete-threshold-seconds must be set together.");
                None
            }
        }
    }
}

#[derive(Args, Clone, Debug)]
#[command(next_help_heading = "Config Options")]
#[group(id = "Config")]
pub struct ConfigOptions {
    /// Path to a configuration file containing the list of supported blockchains.
    #[arg(long = "config")]
    #[arg(env = "PER_CONFIG")]
    #[arg(default_value = "config.yaml")]
    pub config: String,
}

pub type ChainId = String;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct LazerConfig {
    /// The list of price feeds to subscribe to.
    pub price_feeds: Vec<crate::kernel::pyth_lazer::PriceFeed>,
}


#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ConfigMap {
    pub chains: HashMap<ChainId, Config>,
    pub lazer:  LazerConfig,
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
    Svm(ConfigSvm),
}

#[serde_as]
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ConfigSvm {
    /// Id of the express relay program.
    #[serde_as(as = "DisplayFromStr")]
    pub express_relay_program_id:      Pubkey,
    /// RPC endpoint to use for reading from the blockchain.
    pub rpc_read_url:                  String,
    /// RPC endpoint to use for broadcasting transactions
    pub rpc_tx_submission_urls:        Vec<String>,
    /// WS endpoint to use for interacting with the blockchain.
    pub ws_addr:                       String,
    /// Timeout for RPC requests in seconds.
    #[serde(default = "ConfigSvm::default_rpc_timeout_svm")]
    pub rpc_timeout:                   u64,
    #[serde(default)]
    /// Percentile of prioritization fees to query from the `rpc_read_url`.
    /// This should be None unless the RPC `getRecentPrioritizationFees`'s supports the percentile parameter, for example Triton RPC.
    /// It is an integer between 0 and 10000 with 10000 representing 100%.
    pub prioritization_fee_percentile: Option<u64>,
    /// List of accepted token programs for the swap instruction.
    #[serde_as(as = "Vec<DisplayFromStr>")]
    pub accepted_token_programs:       Vec<Pubkey>,
    /// Ordered list of fee tokens, with first being the most preferred.
    #[serde_as(as = "Vec<DisplayFromStr>")]
    pub ordered_fee_tokens:            Vec<Pubkey>,
    /// Whitelisted token mints
    #[serde(default)]
    pub token_whitelist:               TokenWhitelistConfig,
    /// Auction time for the chain (how long to wait before choosing winning bids)
    #[serde(default = "ConfigSvm::default_auction_time", with = "humantime_serde")]
    pub auction_time:                  Duration,
}

impl ConfigSvm {
    pub fn default_rpc_timeout_svm() -> u64 {
        2
    }

    pub fn default_auction_time() -> Duration {
        Duration::from_millis(250)
    }
}

/// Optional whitelist of token mints to allow for getting quotes for
#[serde_as]
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct TokenWhitelistConfig {
    #[serde(default)]
    pub enabled:         bool,
    #[serde(default)]
    #[serde_as(as = "Vec<DisplayFromStr>")]
    pub whitelist_mints: Vec<Pubkey>,
}
