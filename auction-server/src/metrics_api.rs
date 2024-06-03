//! Metrics Server
//!
//! This server serves metrics over /metrics in OpenMetrics format.
use {
    crate::{
        config::RunOptions,
        server::{
            EXIT_CHECK_INTERVAL,
            SHOULD_EXIT,
        },
        state::Store,
    },
    anyhow::Result,
    axum::{
        routing::get,
        Router,
    },
    axum_prometheus::PrometheusMetricLayerBuilder,
    std::sync::{
        atomic::Ordering,
        Arc,
    },
};


pub async fn start_metrics(run_options: RunOptions, store: Arc<Store>) -> Result<()> {
    tracing::info!("Starting Metrics Server...");

    let (_, metric_handle) = PrometheusMetricLayerBuilder::new()
        .with_metrics_from_fn(|| store.metrics_recorder.clone())
        .build_pair();
    let app = Router::new();
    let app = app.route("/metrics", get(|| async move { metric_handle.render() }));

    let listener = tokio::net::TcpListener::bind(&run_options.server.metrics_addr)
        .await
        .unwrap();
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            while !SHOULD_EXIT.load(Ordering::Acquire) {
                tokio::time::sleep(EXIT_CHECK_INTERVAL).await;
            }
            tracing::info!("Shutting down metrics server...");
        })
        .await?;
    Ok(())
}
