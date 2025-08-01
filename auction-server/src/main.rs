#![cfg_attr(test, allow(dead_code))]

use {
    anyhow::Result,
    clap::Parser,
    opentelemetry::KeyValue,
    opentelemetry_otlp::WithExportConfig,
    opentelemetry_sdk::{
        trace::{
            self,
            Sampler,
        },
        Resource,
    },
    per_metrics::{
        is_metrics,
        MetricsLayer,
    },
    server::{
        run_migrations,
        run_migrations_clichouse,
        start_server,
    },
    std::{
        io::IsTerminal,
        time::Duration,
    },
    tracing::{
        Level,
        Metadata,
    },
    tracing_subscriber::{
        filter,
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
mod server;
mod state;

fn is_internal(metadata: &Metadata) -> bool {
    metadata.target().starts_with("auction_server")
}

fn is_loggable(metadata: &Metadata) -> bool {
    metadata.level() <= &Level::INFO
        && metadata.is_event()
        && is_internal(metadata)
        && !is_metrics(metadata, false)
}

fn is_traceable(metadata: &Metadata) -> bool {
    is_internal(metadata) || is_metrics(metadata, true)
}

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
            trace::config()
                .with_sampler(Sampler::ParentBased(Box::new(Sampler::TraceIdRatioBased(
                    std::env::var("TRACE_RATIO")
                        .ok()
                        .and_then(|v| v.parse::<f64>().ok())
                        .unwrap_or(0.05),
                ))))
                .with_resource(Resource::new(vec![
                    KeyValue::new("service.name", "auction-server"),
                    KeyValue::new(
                        "service.env",
                        std::env::var("APP_ENV").unwrap_or("mainnet".to_string()),
                    ),
                ])),
        )
        .install_batch(opentelemetry_sdk::runtime::Tokio)
        .map_err(|e| anyhow::anyhow!("Error initializing open telemetry: {}", e))?;
    let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);

    let registry = tracing_subscriber::registry()
        .with(MetricsLayer.with_filter(filter::filter_fn(|metadata| is_metrics(metadata, false))))
        .with(telemetry.with_filter(filter::filter_fn(is_traceable)));

    if std::io::stderr().is_terminal() {
        registry
            .with(
                log_layer
                    .compact()
                    .with_filter(filter::filter_fn(is_loggable)),
            )
            .init();
    } else {
        registry
            .with(log_layer.json().with_filter(filter::filter_fn(is_loggable)))
            .init();
    }

    // Parse the command line arguments with StructOpt, will exit automatically on `--help` or
    // with invalid arguments.
    match config::Options::parse() {
        config::Options::Run(opts) => start_server(opts).await,
        config::Options::Migrate(opts) => run_migrations(opts).await,
        config::Options::MigrateClickhouse(opts) => run_migrations_clichouse(opts).await,
    }
}
