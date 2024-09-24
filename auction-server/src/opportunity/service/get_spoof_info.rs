use {
    super::{
        ChainTypeEvm,
        Service,
    },
    crate::{
        api::RestError,
        kernel::entities::ChainId,
        opportunity::{
            entities::spoof_info::{
                SpoofInfo,
                SpoofState,
            },
            token_spoof::find_spoof_info,
        },
    },
    ethers::types::Address,
    std::sync::Arc,
};

pub struct GetSpoofInfoInput {
    pub chain_id: ChainId,
    pub token:    Address,
}

impl Service<ChainTypeEvm> {
    /// Find the spoof info for an ERC20 token. This includes the balance slot and the allowance slot
    /// Returns an error if no balance or allowance slot is found
    /// # Arguments
    ///
    /// * `token`: ERC20 token address
    /// * `client`: Client to interact with the blockchain
    #[tracing::instrument(skip_all, fields(token=%input.token))]
    pub async fn get_spoof_info(&self, input: GetSpoofInfoInput) -> Result<SpoofInfo, RestError> {
        let config = self.get_config(&input.chain_id)?;
        match self.repo.get_spoof_info(input.token).await {
            Some(info) => Ok(info),
            None => {
                let result = find_spoof_info(input.token, Arc::new(config.provider.clone()))
                    .await
                    .unwrap_or_else(|e| {
                        tracing::error!("Error finding spoof info: {:?}", e);
                        SpoofInfo {
                            token: input.token,
                            state: SpoofState::UnableToSpoof,
                        }
                    });

                self.repo.add_spoof_info(result.clone()).await;
                Ok(result)
            }
        }
    }
}
