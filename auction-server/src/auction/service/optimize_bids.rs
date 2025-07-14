use {
    super::Service,
    crate::auction::entities::Bid,
    solana_client::rpc_response::RpcResult,
};

impl Service {
    /// Given a list of bids, tries to find the optimal set of bids that can be submitted to the chain
    /// considering the current state of the chain and the pending transactions.
    /// Right now, for simplicity, the method assume the bids are sorted, and tries to submit them in order
    /// and only return the ones that are successfully submitted.
    #[tracing::instrument(skip_all)]
    pub async fn optimize_bids(&self, bids_sorted: &[Bid]) -> RpcResult<Vec<Bid>> {
        let simulator = &self.config.chain_config.simulator;
        simulator.optimize_bids(bids_sorted).await
    }
}
