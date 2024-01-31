use {
    clap::{
        crate_authors,
        crate_description,
        crate_name,
        crate_version,
        Args,
        Parser,
    },
    ethers::abi::Address,
    url::Url,
};

// `Options` is a structup definition to provide clean command-line args for Hermes.
#[derive(Parser, Debug)]
#[command(name = crate_name!())]
#[command(author = crate_authors!())]
#[command(about = crate_description!())]
#[command(version = crate_version!())]
#[allow(clippy::large_enum_variant)]
pub enum Options {
    /// Run the simulator.
    Run(SimulatorOptions),

    /// Setup an already funded searcher account with ERC20 tokens and WETH.
    CreateSearcher(SearcherOptions),

    /// Deploy the token vault contract.
    Deploy(DeployOptions),
}

#[derive(Args, Clone, Debug)]
pub struct RunOptions {
    /// Address and port the server will bind to.
    #[arg(long = "rpc-addr")]
    #[arg(env = "RPC_ADDR")]
    pub rpc_addr: Url,

    /// A 20-byte (40 char) hex encoded Ethereum private key which is used for submitting transactions.
    #[arg(long = "private-key")]
    #[arg(env = "PRIVATE_KEY")]
    pub private_key: String,

    #[arg(long = "token")]
    pub tokens: Vec<Address>,

    #[arg(long = "weth")]
    pub weth: Address,
}

#[derive(Args, Clone, Debug)]
pub struct SimulatorOptions {
    /// Server Options
    #[command(flatten)]
    pub run_options: RunOptions,

    /// The address of the token vault contract to interact with
    #[arg(long = "vault-contract")]
    pub vault_contract: Address,
}

#[derive(Args, Clone, Debug)]
pub struct SearcherOptions {
    /// Server Options
    #[command(flatten)]
    pub run_options: RunOptions,

    /// The address of the liquidation adapter contract to use for approvals
    #[arg(long = "adapter-contract")]
    pub adapter_contract: Address,
}

#[derive(Args, Clone, Debug)]
pub struct DeployOptions {
    /// Address and port the server will bind to.
    #[arg(long = "rpc-addr")]
    #[arg(env = "RPC_ADDR")]
    pub rpc_addr: Url,

    /// A 20-byte (40 char) hex encoded Ethereum private key which is used for submitting transactions.
    #[arg(long = "private-key")]
    #[arg(env = "PRIVATE_KEY")]
    pub private_key: String,

    /// The per contract address
    #[arg(long = "per-contract")]
    pub per_contract: Address,

    /// The oracle contract address
    #[arg(long = "oracle-contract")]
    pub oracle_contract: Address,
}
