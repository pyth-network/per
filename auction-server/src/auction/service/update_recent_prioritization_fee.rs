use {
    super::Service,
    crate::{
        api::RestError,
        kernel::entities::Svm,
    },
    solana_client::rpc_response::RpcPrioritizationFee,
};

pub const RECENT_FEES_SLOT_WINDOW: usize = 12;

impl Service<Svm> {
    /// Returns an estimate of recent prioritization fees.
    /// For each of the last 150 slots, client returns the `config.prioritization_fee_percentile`th percentile
    /// of prioritization fees for transactions that landed in that slot.
    /// The median of such values for the `RECENT_FEES_SLOT_WINDOW` most recent slots is returned.
    pub async fn update_recent_prioritization_fee(&self) -> Result<u64, RestError> {
        let accounts: Vec<String> = vec![];
        let mut args: Vec<serde_json::Value> = vec![
            serde_json::to_value(accounts).expect("Failed to serialize empty list of accounts")
        ];

        if let Some(percentile) = self.config.chain_config.prioritization_fee_percentile {
            args.push(serde_json::json!({ "percentile": percentile }));
        }

        fn median(values: &mut [u64]) -> u64 {
            let mid = values.len() / 2;
            *values.select_nth_unstable(mid).1
        }

        let fee = self
            .config
            .chain_config
            .client
            .send(
                solana_client::rpc_request::RpcRequest::GetRecentPrioritizationFees,
                serde_json::Value::from(args),
            )
            .await
            .map(|mut values: Vec<RpcPrioritizationFee>| {
                values.sort_by(|a, b| b.slot.cmp(&a.slot));
                median(
                    &mut values
                        .iter()
                        .take(RECENT_FEES_SLOT_WINDOW)
                        .map(|fee| fee.prioritization_fee)
                        .collect::<Vec<u64>>(),
                )
            })
            .map_err(|e| {
                tracing::error!(error = ?e, "Failed to get prioritization fee");
                RestError::TemporarilyUnavailable
            })?;

        self.repo.add_recent_prioritization_fee(fee).await;
        Ok(fee)
    }
}
