use {
    crate::config::ChainId,
    anyhow::Result,
    axum::async_trait,
    axum_prometheus::metrics,
    ethers::{
        prelude::Http,
        providers::{
            HttpClientError,
            JsonRpcClient,
            Provider,
        },
    },
    std::time::{
        Duration,
        Instant,
    },
};

#[derive(Debug, Clone)]
pub struct TracedClient {
    inner:    Http,
    chain_id: ChainId,
}

#[async_trait]
impl JsonRpcClient for TracedClient {
    type Error = HttpClientError;

    async fn request<
        T: serde::Serialize + Send + Sync + std::fmt::Debug,
        R: serde::de::DeserializeOwned + Send,
    >(
        &self,
        method: &str,
        params: T,
    ) -> Result<R, HttpClientError> {
        let start = Instant::now();
        let res = self.inner.request(method, params).await;

        let result_label = match &res {
            Ok(_) => "success",
            Err(_) => "error",
        };

        let labels = [
            ("chain_id", self.chain_id.clone()),
            ("method", method.to_string()),
            ("result", result_label.to_string()),
        ];

        let latency = start.elapsed().as_secs_f64();
        metrics::counter!("rpc_requests_total", &labels).increment(1);
        metrics::histogram!("rpc_requests_duration_seconds", &labels).record(latency);
        res
    }
}

impl TracedClient {
    pub fn new(chain_id: ChainId, url: &str, timeout: u64) -> Result<Provider<TracedClient>> {
        let url = reqwest::Url::parse(url)?;
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(timeout))
            .build()?;
        Ok(Provider::new(TracedClient {
            inner: Http::new_with_client(url, client),
            chain_id,
        }))
    }
}
