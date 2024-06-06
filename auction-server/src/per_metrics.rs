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
    tracing::{
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

#[derive(Debug, Clone)]
pub struct MetricsLayerData {
    category:   String,
    started_at: std::time::Instant,
    result:     String,
    name:       String,
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
        }
    }
}

impl Default for MetricsLayerData {
    fn default() -> MetricsLayerData {
        MetricsLayerData {
            category:   "unknown".to_string(),
            started_at: Instant::now(),
            result:     "success".to_string(),
            name:       "unknown".to_string(),
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

pub fn is_metrics(metadata: &Metadata) -> bool {
    metadata.target().starts_with("metrics")
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
                span.extensions_mut().replace(data.clone());
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
                    let labels = [("name", data.name.clone()), ("result", data.result.clone())];
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
