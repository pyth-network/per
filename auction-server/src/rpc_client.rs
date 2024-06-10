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
            JsonRpcError,
            Provider,
            ProviderError,
            RpcError,
        },
    },
    std::{
        fmt,
        str::FromStr,
        time::Instant,
    },
    thiserror::Error,
    tokio::time::timeout,
};

#[derive(Debug, Clone)]
pub struct RPCClient {
    inner:    Http,
    chain_id: ChainId,
}

#[derive(Error, Debug)]
pub enum RPCClientError {
    Timeout,
    HttpClientError(HttpClientError),
}

impl fmt::Display for RPCClientError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl RpcError for RPCClientError {
    fn as_error_response(&self) -> Option<&JsonRpcError> {
        if let RPCClientError::HttpClientError(err) = self {
            err.as_error_response()
        } else {
            None
        }
    }

    fn as_serde_error(&self) -> Option<&serde_json::Error> {
        match self {
            RPCClientError::HttpClientError(err) => err.as_serde_error(),
            _ => None,
        }
    }
}

impl From<RPCClientError> for ProviderError {
    fn from(src: RPCClientError) -> Self {
        match src {
            RPCClientError::HttpClientError(err) => err.into(),
            RPCClientError::Timeout => {
                ProviderError::CustomError("rpc request timeout".to_string())
            }
        }
    }
}

#[async_trait]
impl JsonRpcClient for RPCClient {
    type Error = RPCClientError;

    async fn request<
        T: serde::Serialize + Send + Sync + std::fmt::Debug,
        R: serde::de::DeserializeOwned + Send,
    >(
        &self,
        method: &str,
        params: T,
    ) -> Result<R, RPCClientError> {
        let start = Instant::now();

        let timeout_duration = std::time::Duration::from_secs(10);
        let (res, result_label) =
            match timeout(timeout_duration, self.inner.request(method, params)).await {
                Ok(Ok(res)) => (Ok(res), "success"),
                Ok(Err(err)) => (Err(RPCClientError::HttpClientError(err)), "error"),
                Err(_) => (Err(RPCClientError::Timeout), "timeout"),
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

impl RPCClient {
    pub fn new(chain_id: ChainId, url: &str) -> Result<Provider<RPCClient>> {
        Ok(Provider::new(RPCClient {
            inner: Http::from_str(url)?,
            chain_id,
        }))
    }
}
