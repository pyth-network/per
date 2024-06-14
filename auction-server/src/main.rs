use {
    anyhow::Result,
    clap::Parser,
    per_metrics::{
        is_metrics,
        MetricsLayer,
    },
    server::start_server,
    std::io::IsTerminal,
    tracing_subscriber::{
        filter::{
            self,
            LevelFilter,
        },
        layer::SubscriberExt,
        util::SubscriberInitExt,
        Layer,
    },
};

mod api;
mod auction;
mod config;
mod models;
mod opportunity_adapter;
mod per_metrics;
mod serde;
mod server;
mod state;
mod subwallet;
mod token_spoof;
mod traced_client;


#[tokio::main]
async fn main() -> Result<()> {
    // Initialize a Tracing Subscriber
    let log_layer = tracing_subscriber::fmt::layer()
        .with_file(false)
        .with_line_number(true)
        .with_thread_ids(true)
        .with_target(true)
        .with_ansi(std::io::stderr().is_terminal());

    let registry = tracing_subscriber::registry()
        .with(MetricsLayer.with_filter(filter::filter_fn(is_metrics)));

    if std::io::stderr().is_terminal() {
        registry
            .with(
                log_layer
                    .compact()
                    .with_filter(LevelFilter::INFO)
                    .with_filter(filter::filter_fn(|metadata| !is_metrics(metadata))),
            )
            .init();
    } else {
        registry
            .with(
                log_layer
                    .json()
                    .with_filter(LevelFilter::INFO)
                    .with_filter(filter::filter_fn(|metadata| !is_metrics(metadata))),
            )
            .init();
    }

    // Parse the command line arguments with StructOpt, will exit automatically on `--help` or
    // with invalid arguments.
    match config::Options::parse() {
        config::Options::Run(opts) => start_server(opts).await,
        config::Options::SyncSubwallets(opts) => subwallet::sync_subwallets(opts).await,
    }
}
