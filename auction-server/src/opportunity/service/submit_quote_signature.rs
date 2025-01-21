use {
    crate::{
        api::RestError,
        auction::{
            entities::BidChainData,
            service::auction_manager::AuctionManager,
        },
        opportunity::{
            entities::SubmitQuoteSignatureInput,
            service::{
                ChainTypeSvm,
                Service,
            },
        },
    },
    solana_sdk::transaction::VersionedTransaction,
    std::time::Duration,
    time::OffsetDateTime,
};

impl Service<ChainTypeSvm> {
    #[tracing::instrument(skip_all)]
    pub async fn submit_quote_signature(
        &self,
        input: SubmitQuoteSignatureInput,
    ) -> Result<VersionedTransaction, RestError> {
        let tx = self.repo.get_swap_transaction(input.opportunity_id).await?;
        let mut bid = tx.ok_or(RestError::QuoteNotFound)?;
        bid.chain_data.transaction.signatures[1] = input.signature;
        let transaction = bid.chain_data.transaction.clone();
        let config = self.get_config(&bid.chain_id)?;
        let auction_service = config.get_auction_service().await;
        let swap_data =
            auction_service.extract_swap_data_from_transaction(&bid.chain_data.transaction)?;
        let deadline = OffsetDateTime::from_unix_timestamp(swap_data.deadline).map_err(|e| {
            RestError::BadParameters(format!(
                "Invalid deadline: {:?} {:?}",
                swap_data.deadline, e
            ))
        })?;
        let minimum_deadline = OffsetDateTime::now_utc() + Duration::from_secs(5);
        if deadline < minimum_deadline {
            return Err(RestError::BadParameters(
                "User signature received too late".to_string(),
            ));
        }
        auction_service
            .submit_bids(bid.chain_data.get_permission_key(), vec![bid])
            .await
            .map_err(|_| RestError::TemporarilyUnavailable)?;

        Ok(transaction)
    }
}
