use {
    super::{
        ChainTypeSvm,
        Service,
    },
    crate::{
        api::RestError,
        kernel::entities::ChainId,
        opportunity::service::ConfigSvm,
    },
    ::express_relay as express_relay_svm,
    anchor_lang::AccountDeserialize,
    express_relay::state::ExpressRelayMetadata,
    solana_sdk::{
        account::Account,
        pubkey::Pubkey,
    },
};

pub struct GetExpressRelayMetadata {
    pub chain_id: ChainId,
}

// TODO: Move this to kernel
impl Service<ChainTypeSvm> {
    /// Fetches the express relay program metadata for fee calculations
    /// Uses one hour cache to avoid repeated RPC calls
    pub async fn get_express_relay_metadata(
        &self,
        input: GetExpressRelayMetadata,
    ) -> Result<ExpressRelayMetadata, RestError> {
        let token_program_query = self.repo.query_express_relay_metadata().await;
        let metadata = match token_program_query {
            Some(metadata) => metadata,
            None => {
                let metadata_account = self
                    .fetch_express_relay_metadata_account(&input.chain_id)
                    .await?;
                let metadata = ExpressRelayMetadata::try_deserialize(&mut metadata_account.data.as_slice())
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

    async fn fetch_express_relay_metadata_account(
        &self,
        chain_id: &ChainId,
    ) -> Result<Account, RestError> {
        let config = self.get_config(chain_id)?;
        let metadata_address = Self::calculate_metadata_address(config).await;
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
        Ok(metadata_account)
    }

    pub async fn calculate_metadata_address(config: &ConfigSvm) -> Pubkey {
        let program_id = config
            .get_auction_service()
            .await
            .get_express_relay_program_id();
        let seed = express_relay_svm::state::SEED_METADATA;
        Pubkey::find_program_address(&[seed], &program_id).0
    }
}
