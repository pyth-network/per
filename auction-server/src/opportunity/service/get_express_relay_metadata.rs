use {
    super::{
        ChainTypeSvm,
        Service,
    },
    crate::{
        api::RestError,
        kernel::entities::ChainId,
    },
    ::express_relay as express_relay_svm,
    anchor_lang::AccountDeserialize,
    express_relay::state::ExpressRelayMetadata,
    solana_sdk::pubkey::Pubkey,
};

pub struct GetExpressRelayMetadata {
    pub chain_id: ChainId,
}

impl Service<ChainTypeSvm> {
    /// Fetches the express relay program metadata for fee calculations
    /// Uses one hour cache to avoid repeated RPC calls
    pub async fn get_express_relay_metadata(
        &self,
        input: GetExpressRelayMetadata,
    ) -> Result<ExpressRelayMetadata, RestError> {
        let config = self.get_config(&input.chain_id)?;
        let program_id = config.get_auction_service().await.get_program_id();
        let seed = express_relay_svm::state::SEED_METADATA;
        let metadata_address = Pubkey::find_program_address(&[seed], &program_id).0;
        tracing::info!("Express relay metadata address: {}", metadata_address);
        let token_program_query = self.repo.query_express_relay_metadata().await;
        let metadata = match token_program_query {
            Some(metadata) => metadata,
            None => {
                let metadata_account = config
                    .rpc_client
                    .get_account(&metadata_address)
                    .await
                    .map_err(|err| {
                        tracing::error!(error=?err,
                            "Failed to retrieve express relay metadata account from rpc"
                        );
                        RestError::TemporarilyUnavailable
                    })?;
                let metadata = express_relay_svm::state::ExpressRelayMetadata::try_deserialize(&mut metadata_account.data.as_slice())
                    .map_err(|err| {
                        tracing::error!(error = ?err,"Failed to deserialize express relay metadata account");
                        RestError::TemporarilyUnavailable
                    })?;
                self.repo
                    .cache_express_relay_metadata(metadata.clone())
                    .await;
                metadata
            }
        };

        Ok(metadata)
    }
}
