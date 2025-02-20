use {
    super::Service,
    crate::{
        auction::entities::Bid,
        kernel::entities::Svm,
    },
    solana_client::rpc_response::{
        Response,
        RpcResult,
    },
};

impl Service<Svm> {
    /// Given a list of bids, tries to find the optimal set of bids that can be submitted to the chain
    /// considering the current state of the chain and the pending transactions.
    /// Right now, for simplicity, the method assume the bids are sorted, and tries to submit them in order
    /// and only return the ones that are successfully submitted.
    pub async fn optimize_bids(&self, bids_sorted: &[Bid<Svm>]) -> RpcResult<Vec<Bid<Svm>>> {
        let simulator = &self.config.chain_config.simulator;
        let pending_txs = simulator.fetch_pending_and_remove_old_txs().await;
        let txs_to_fetch = pending_txs
            .iter()
            .chain(bids_sorted.iter().map(|bid| &bid.chain_data.transaction))
            .cloned()
            .collect::<Vec<_>>();
        let accounts_config_with_context =
            simulator.fetch_tx_accounts_via_rpc(&txs_to_fetch).await?;
        let mut svm = simulator.setup_lite_svm(&accounts_config_with_context);

        pending_txs.into_iter().for_each(|tx| {
            let _ = svm.send_transaction(tx);
        });
        let mut res = vec![];
        for bid in bids_sorted {
            if svm
                .send_transaction(bid.chain_data.transaction.clone())
                .is_ok()
            {
                res.push(bid.clone());
            }
        }
        Ok(Response {
            value:   res,
            context: accounts_config_with_context.context,
        })
    }
}
