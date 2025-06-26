use {
    crate::{
        config::RunOptions,
        server::{
            EXIT_CHECK_INTERVAL,
            SHOULD_EXIT,
        },
        state::ServerState,
    },
    anyhow::Result,
    axum::{
        routing::get,
        Router,
    },
    axum_prometheus::{
        metrics,
        PrometheusMetricLayerBuilder,
    },
    std::{
        fmt::Debug,
        sync::{
            atomic::Ordering,
            Arc,
        },
        time::Instant,
    },
    tokio_metrics::RuntimeMonitor,
    tracing::{
        error,
        field::{
            Field,
            Visit,
        },
        span::Record,
        Id,
        Metadata,
    },
    tracing_subscriber::{
        layer::Context,
        Layer,
    },
};

pub const TRANSACTION_LANDING_TIME_SVM_METRIC: &str = "transaction_landing_time_seconds_svm";
pub const TRANSACTION_LANDING_TIME_SVM_BUCKETS: &[f64; 16] = &[
    0.1, 0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0, 2.25, 2.5, 3.75, 5.0, 10.0, 20.0, 40.0,
];

pub const SUBMIT_QUOTE_DEADLINE_BUFFER_METRIC: &str = "submit_quote_deadline_buffer";
pub const SUBMIT_QUOTE_DEADLINE_BUFFER_BUCKETS: &[f64; 20] = &[
    -5.0, -2.0, -1.0, 0.0, 1.0, 2.0, 2.5, 3.0, 3.5, 4.0, 4.5, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 20.0,
    30.0, 50.0,
];
pub const SUBMIT_QUOTE_DEADLINE_TOTAL: &str = "submit_quote_deadline_total";

pub const QUOTE_VALIDATION_TOTAL: &str = "quote_validation_total";

#[derive(Debug, Clone)]
pub struct MetricsLayerData {
    category:   String,
    started_at: std::time::Instant,
    result:     String,
    name:       String,
    profile:    String,
}

pub struct MetricsLayer;

impl Visit for MetricsLayerData {
    fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
        if field.name() == "result" {
            self.result = format!("{:?}", value);
        }
    }
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "category" {
            self.category = value.to_string();
        } else if field.name() == "result" {
            self.result = value.to_string();
        } else if field.name() == "name" {
            self.name = value.to_string();
        } else if field.name() == "profile" {
            self.profile = value.to_string();
        }
    }
}

impl Default for MetricsLayerData {
    fn default() -> MetricsLayerData {
        MetricsLayerData {
            category:   "unknown".to_string(),
            started_at: Instant::now(),
            result:     "unknown".to_string(),
            name:       "unknown".to_string(),
            profile:    "unknown".to_string(),
        }
    }
}

impl MetricsLayerData {
    fn new(name: String) -> MetricsLayerData {
        MetricsLayerData {
            name,
            ..MetricsLayerData::default()
        }
    }
}

pub fn is_metrics(metadata: &Metadata, check_tracing_enabled: bool) -> bool {
    let tracing_check = !check_tracing_enabled
        || metadata
            .fields()
            .iter()
            .any(|f| f.name() == "tracing_enabled");
    tracing_check && (metadata.target().starts_with("metrics"))
}

impl<S> Layer<S> for MetricsLayer
where
    S: tracing::Subscriber,
    S: for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
{
    fn on_new_span(
        &self,
        attrs: &tracing::span::Attributes<'_>,
        id: &tracing::span::Id,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        match ctx.span(id) {
            Some(span) => {
                let mut data = MetricsLayerData::new(span.metadata().name().to_string());
                attrs.record(&mut data);
                span.extensions_mut().replace(data);
            }
            None => tracing::error!("span not found: {:?}", id),
        }
    }

    fn on_record(&self, id: &Id, values: &Record<'_>, ctx: Context<'_, S>) {
        match ctx.span(id) {
            Some(span) => {
                let mut extension_mut = span.extensions_mut();
                match extension_mut.get_mut::<MetricsLayerData>() {
                    Some(data) => {
                        values.record(data);
                    }
                    None => {
                        tracing::warn!("metrics layer not found for span: {:?}", id);
                        extension_mut.replace(MetricsLayerData::default());
                    }
                }
            }
            None => tracing::error!("span not found: {:?}", id),
        }
    }

    fn on_close(&self, id: Id, ctx: Context<'_, S>) {
        match ctx.span(&id) {
            Some(span) => match span.extensions().get::<MetricsLayerData>() {
                Some(data) => {
                    let latency = (Instant::now() - data.started_at).as_secs_f64();
                    let labels = [
                        ("name", data.name.clone()),
                        ("result", data.result.clone()),
                        ("profile", data.profile.clone()),
                    ];
                    metrics::histogram!(format!("{}_duration_seconds", data.category), &labels)
                        .record(latency);
                    metrics::counter!(format!("{}_total", data.category), &labels).increment(1);
                }
                None => {
                    tracing::warn!("metrics layer not found for span: {:?}", id);
                }
            },
            None => tracing::error!("span not found: {:?}", id),
        }
    }
}

pub async fn update_tokio_runtime_metrics(runtime_monitor: &RuntimeMonitor) {
    let Some(metrics) = runtime_monitor.intervals().next() else {
        error!("No tokio runtime metrics available");
        return;
    };

    // Worker metrics
    metrics::gauge!("tokio_workers_count").set(metrics.workers_count as f64);

    // Park count metrics
    metrics::gauge!("tokio_total_park_count").set(metrics.total_park_count as f64);
    metrics::gauge!("tokio_max_park_count").set(metrics.max_park_count as f64);
    metrics::gauge!("tokio_min_park_count").set(metrics.min_park_count as f64);

    // Poll duration metrics
    metrics::gauge!("tokio_mean_poll_duration_ns")
        .set(metrics.mean_poll_duration.as_nanos() as f64);
    metrics::gauge!("tokio_mean_poll_duration_worker_min_ns")
        .set(metrics.mean_poll_duration_worker_min.as_nanos() as f64);
    metrics::gauge!("tokio_mean_poll_duration_worker_max_ns")
        .set(metrics.mean_poll_duration_worker_max.as_nanos() as f64);

    // Noop metrics
    metrics::gauge!("tokio_total_noop_count").set(metrics.total_noop_count as f64);
    metrics::gauge!("tokio_max_noop_count").set(metrics.max_noop_count as f64);
    metrics::gauge!("tokio_min_noop_count").set(metrics.min_noop_count as f64);

    // Steal metrics
    metrics::gauge!("tokio_total_steal_count").set(metrics.total_steal_count as f64);
    metrics::gauge!("tokio_max_steal_count").set(metrics.max_steal_count as f64);
    metrics::gauge!("tokio_min_steal_count").set(metrics.min_steal_count as f64);
    metrics::gauge!("tokio_total_steal_operations").set(metrics.total_steal_operations as f64);
    metrics::gauge!("tokio_max_steal_operations").set(metrics.max_steal_operations as f64);
    metrics::gauge!("tokio_min_steal_operations").set(metrics.min_steal_operations as f64);

    // Schedule metrics
    metrics::gauge!("tokio_num_remote_schedules").set(metrics.num_remote_schedules as f64);
    metrics::gauge!("tokio_total_local_schedule_count")
        .set(metrics.total_local_schedule_count as f64);
    metrics::gauge!("tokio_max_local_schedule_count").set(metrics.max_local_schedule_count as f64);
    metrics::gauge!("tokio_min_local_schedule_count").set(metrics.min_local_schedule_count as f64);

    // Overflow metrics
    metrics::gauge!("tokio_total_overflow_count").set(metrics.total_overflow_count as f64);
    metrics::gauge!("tokio_max_overflow_count").set(metrics.max_overflow_count as f64);
    metrics::gauge!("tokio_min_overflow_count").set(metrics.min_overflow_count as f64);

    // Polls metrics
    metrics::gauge!("tokio_total_polls_count").set(metrics.total_polls_count as f64);
    metrics::gauge!("tokio_max_polls_count").set(metrics.max_polls_count as f64);
    metrics::gauge!("tokio_min_polls_count").set(metrics.min_polls_count as f64);

    // Busy duration metrics
    metrics::gauge!("tokio_total_busy_duration_ns")
        .set(metrics.total_busy_duration.as_nanos() as f64);
    metrics::gauge!("tokio_max_busy_duration_ns").set(metrics.max_busy_duration.as_nanos() as f64);
    metrics::gauge!("tokio_min_busy_duration_ns").set(metrics.min_busy_duration.as_nanos() as f64);

    // Queue depth metrics
    metrics::gauge!("tokio_global_queue_depth").set(metrics.global_queue_depth as f64);
    metrics::gauge!("tokio_total_local_queue_depth").set(metrics.total_local_queue_depth as f64);
    metrics::gauge!("tokio_max_local_queue_depth").set(metrics.max_local_queue_depth as f64);
    metrics::gauge!("tokio_min_local_queue_depth").set(metrics.min_local_queue_depth as f64);
    metrics::gauge!("tokio_blocking_queue_depth").set(metrics.blocking_queue_depth as f64);

    // Task and thread metrics
    metrics::gauge!("tokio_live_tasks_count").set(metrics.live_tasks_count as f64);
    metrics::gauge!("tokio_blocking_threads_count").set(metrics.blocking_threads_count as f64);
    metrics::gauge!("tokio_idle_blocking_threads_count")
        .set(metrics.idle_blocking_threads_count as f64);

    // Other metrics
    metrics::gauge!("tokio_elapsed_us").set(metrics.elapsed.as_micros() as f64);
    metrics::gauge!("tokio_budget_forced_yield_count")
        .set(metrics.budget_forced_yield_count as f64);
    metrics::gauge!("tokio_io_driver_ready_count").set(metrics.io_driver_ready_count as f64);
}

pub async fn start_metrics(run_options: RunOptions, server_state: Arc<ServerState>) -> Result<()> {
    tracing::info!("Starting Metrics Server...");

    let (_, metric_handle) = PrometheusMetricLayerBuilder::new()
        .with_metrics_from_fn(|| server_state.metrics_recorder.clone())
        .build_pair();
    let app = Router::new();
    let app = app.route("/metrics", get(|| async move { metric_handle.render() }));

    let listener = tokio::net::TcpListener::bind(&run_options.server.metrics_addr).await?;
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
