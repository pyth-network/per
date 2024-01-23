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
    std::{
        collections::HashMap,
        fs,
        net::SocketAddr,
    },
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
    Run(RunOptions),
}

#[derive(Args, Clone, Debug)]
pub struct RunOptions {

    /// Address and port the server will bind to.
    #[arg(long = "rpc-addr")]
    #[arg(env = "RPC_ADDR")]
    pub listen_addr: SocketAddr,

    /// A 20-byte (40 char) hex encoded Ethereum private key which is used for submitting transactions.
    #[arg(long = "private-key")]
    #[arg(env = "PRIVATE_KEY")]
    pub private_key: String,
}
