use {
    clap::Args,
    std::net::SocketAddr,
};

const DEFAULT_LISTEN_ADDR: &str = "127.0.0.1:9000";
const DEFAULT_METRICS_ADDR: &str = "127.0.0.1:9001";

#[derive(Args, Clone, Debug)]
#[command(next_help_heading = "Server Options")]
#[group(id = "Server")]
pub struct Options {
    /// Address and port the server will bind to.
    #[arg(long = "listen-addr")]
    #[arg(default_value = DEFAULT_LISTEN_ADDR)]
    #[arg(env = "LISTEN_ADDR")]
    pub listen_addr:  SocketAddr,
    /// database url for persistent storage.
    #[arg(long = "database-url")]
    #[arg(env = "DATABASE_URL")]
    pub database_url: String,
    /// Address and port the metrics will bind to.
    #[arg(long = "metrics-addr")]
    #[arg(default_value = DEFAULT_METRICS_ADDR)]
    #[arg(env = "METRICS_ADDR")]
    pub metrics_addr: SocketAddr,
}
