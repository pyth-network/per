use {
    crate::server::run_migrations,
    anyhow::Result,
    clap::Parser,
    opentelemetry::KeyValue,
    opentelemetry_otlp::WithExportConfig,
    opentelemetry_sdk::{
        trace,
        Resource,
    },
    per_metrics::{
        is_metrics,
        MetricsLayer,
    },
    server::start_server,
    std::{
        io::IsTerminal,
        time::Duration,
    },
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
mod kernel;
mod models;
mod opportunity;
mod per_metrics;
mod serde;
mod server;
mod simulator;
mod state;
mod subwallet;
mod traced_client;
mod traced_sender_svm;
mod watcher;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize a Tracing Subscriber
    let log_layer = tracing_subscriber::fmt::layer()
        .with_file(false)
        .with_line_number(true)
        .with_thread_ids(true)
        .with_target(true)
        .with_ansi(std::io::stderr().is_terminal());


    // Will use env variable OTEL_EXPORTER_OTLP_ENDPOINT or defaults to 127.0.0.1:4317
    let otlp_exporter = opentelemetry_otlp::new_exporter()
        .tonic()
        .with_timeout(Duration::from_secs(3));
    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(otlp_exporter)
        .with_trace_config(
            trace::config().with_resource(Resource::new(vec![KeyValue::new(
                "service.name",
                "auction-server",
            )])),
        )
        .install_batch(opentelemetry_sdk::runtime::Tokio)
        .map_err(|e| anyhow::anyhow!("Error initializing open telemetry: {}", e))?;
    let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);

    let registry = tracing_subscriber::registry()
        .with(MetricsLayer.with_filter(filter::filter_fn(is_metrics)))
        .with(telemetry.with_filter(filter::filter_fn(|metadata| !is_metrics(metadata))));

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
        config::Options::Migrate(opts) => run_migrations(opts).await,
    }
}
