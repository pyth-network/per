use {
    crate::config::ChainId,
    axum::async_trait,
    axum_prometheus::metrics,
    solana_client::{
        client_error,
        nonblocking::rpc_client::RpcClient,
        rpc_client::RpcClientConfig,
        rpc_request::RpcRequest,
        rpc_sender::{
            RpcSender,
            RpcTransportStats,
        },
    },
    solana_rpc_client::http_sender::HttpSender,
    std::time::{
        Duration,
        Instant,
    },
};

pub struct TracedSenderSvm {
    sender:   HttpSender,
    chain_id: ChainId,
}

#[async_trait]
impl RpcSender for TracedSenderSvm {
    async fn send(
        &self,
        request: RpcRequest,
        params: serde_json::Value,
    ) -> client_error::Result<serde_json::Value> {
        let start = Instant::now();
        let res = self.sender.send(request, params).await;
        let result_label = match &res {
            Ok(_) => "success",
            Err(e) => {
                tracing::error!(error = ?e, "svm rpc request failed");
                "error"
            }
        };

        let labels = [
            ("chain_id", self.chain_id.clone()),
            ("method", request.to_string()),
            ("result", result_label.to_string()),
        ];

        let latency = start.elapsed().as_secs_f64();
        metrics::counter!("rpc_requests_total_svm", &labels).increment(1);
        metrics::histogram!("rpc_requests_duration_seconds_svm", &labels).record(latency);
        res
    }

    fn get_transport_stats(&self) -> RpcTransportStats {
        self.sender.get_transport_stats()
    }

    fn url(&self) -> String {
        self.sender.url()
    }
}

impl TracedSenderSvm {
    pub fn new_client(
        chain_id: ChainId,
        url: &str,
        timeout: u64,
        config: RpcClientConfig,
    ) -> RpcClient {
        let sender = HttpSender::new_with_timeout(url, Duration::from_secs(timeout));
        RpcClient::new_sender(TracedSenderSvm { sender, chain_id }, config)
    }
}

#[cfg(test)]
pub mod tests {
    use {
        super::*,
        mockall::mock,
    };

    mock!(
        pub RpcClient {}
        #[async_trait]
        impl RpcSender for RpcClient {
            async fn send(&self, request: RpcRequest, params: serde_json::Value) -> client_error::Result<serde_json::Value>;
            fn get_transport_stats(&self) -> RpcTransportStats ;
            fn url(&self) -> String;
        }
    );
}
