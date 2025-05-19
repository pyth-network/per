use {
    clap::Args,
    std::net::SocketAddr,
};

const DEFAULT_LISTEN_ADDR: &str = "127.0.0.1:9000";
const DEFAULT_METRICS_ADDR: &str = "127.0.0.1:9001";
const DEFAULT_DATABASE_CONNECTIONS: &str = "10";
const DEFAULT_REQUESTER_IP_HEADER_NAME: &str = "X-Forwarded-For";

#[derive(Args, Clone, Debug)]
pub struct ClickhouseConfig {
    #[arg(long = "clickhouse-url")]
    #[arg(env = "CLICKHOUSE_URL")]
    pub clickhouse_url:      String,
    #[arg(long = "clickhouse-name")]
    #[arg(env = "CLICKHOUSE_NAME")]
    pub clickhouse_name:     String,
    #[arg(long = "clickhouse-user")]
    #[arg(env = "CLICKHOUSE_USER")]
    pub clickhouse_user:     String,
    #[arg(long = "clickhouse-password")]
    #[arg(env = "CLICKHOUSE_PASSWORD")]
    pub clickhouse_password: String,
}


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
    #[arg(default_value = DEFAULT_DATABASE_CONNECTIONS)]
    #[arg(env = "DATABASE_MAX_CONNECTIONS")]
    pub database_max_connections: u32,
    /// database min connections.
    #[arg(long = "database-min-connections")]
    #[arg(default_value = DEFAULT_DATABASE_CONNECTIONS)]
    #[arg(env = "DATABASE_MIN_CONNECTIONS")]
    pub database_min_connections: u32,
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
    /// Clickhouse database config to run the migrations for.
    #[command(flatten)]
    pub clickhouse_config:        ClickhouseConfig,
}
