use {
    clap::Args,
    std::net::SocketAddr,
};

const DEFAULT_LISTEN_ADDR: &str = "127.0.0.1:9000";
const DEFAULT_METRICS_ADDR: &str = "127.0.0.1:9001";
const DEFAULT_DATABASE_MAX_CONNECTIONS: &str = "10";
const DEFAULT_REQUESTER_IP_HEADER_NAME: &str = "X-Forwarded-For";

#[derive(Args, Clone, Debug)]
#[command(next_help_heading = "Server Options")]
#[group(id = "Server")]
pub struct Options {
    /// Address and port the server will bind to.
    #[arg(long = "listen-addr")]
    #[arg(default_value = DEFAULT_LISTEN_ADDR)]
    #[arg(env = "LISTEN_ADDR")]
    pub listen_addr:              SocketAddr,
    /// database url for persistent storage.
    #[arg(long = "database-url")]
    #[arg(env = "DATABASE_URL")]
    pub database_url:             String,
    /// database max connections.
    #[arg(long = "database-max-connections")]
    #[arg(default_value = DEFAULT_DATABASE_MAX_CONNECTIONS)]
    #[arg(env = "DATABASE_MAX_CONNECTIONS")]
    pub database_max_connections: u32,
    /// Address and port the metrics will bind to.
    #[arg(long = "metrics-addr")]
    #[arg(default_value = DEFAULT_METRICS_ADDR)]
    #[arg(env = "METRICS_ADDR")]
    pub metrics_addr:             SocketAddr,
    /// The header name to use for the requester IP address.
    #[arg(long = "requester-ip-header-name")]
    #[arg(default_value = DEFAULT_REQUESTER_IP_HEADER_NAME)]
    #[arg(env = "REQUESTER_IP_HEADER_NAME")]
    pub requester_ip_header_name: String,
}
