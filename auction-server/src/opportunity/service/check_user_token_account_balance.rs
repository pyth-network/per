use {
    super::{
        get_token_program::GetTokenProgramInput,
        ChainTypeSvm,
        Service,
    },
    crate::{
        api::RestError,
        kernel::entities::ChainId,
    },
    solana_sdk::{
        commitment_config::CommitmentConfig,
        pubkey::Pubkey,
    },
    spl_associated_token_account::get_associated_token_address_with_program_id,
    spl_token_2022::{
        extension::StateWithExtensions as TokenAccountWithExtensions,
        state::Account as TokenAccount,
    },
};

pub struct CheckUserTokenBalanceInput {
    pub chain_id:    ChainId,
    pub user:        Pubkey,
    pub mint_user:   Pubkey,
    pub amount_user: u64,
}

impl Service<ChainTypeSvm> {
    pub async fn check_user_token_balance(
        &self,
        input: CheckUserTokenBalanceInput,
    ) -> Result<bool, RestError> {
        let config = self.get_config(&input.chain_id)?;

        let native_amount_user = if input.mint_user == spl_token::native_mint::id() {
            config
                .rpc_client
                .get_account_with_commitment(&input.user, CommitmentConfig::processed())
                .await
                .map_err(|err| {
                    tracing::error!(error = ?err, "Failed to get user wallet");
                    RestError::TemporarilyUnavailable
                })?
                .value
                .map(|account| account.lamports)
                .unwrap_or_default()
        } else {
            0
        };

        let user_ata_mint_user = get_associated_token_address_with_program_id(
            &input.user,
            &input.mint_user,
            &self
                .get_token_program(GetTokenProgramInput {
                    chain_id: input.chain_id.clone(),
                    mint:     input.mint_user,
                })
                .await?,
        );
        let amount_user: u64 = config
            .rpc_client
            .get_account_with_commitment(&user_ata_mint_user, CommitmentConfig::processed())
            .await
            .map_err(|err| {
                tracing::error!(error = ?err, "Failed to get user token account");
                RestError::TemporarilyUnavailable
            })?
            .value
            .map(|account| {
                TokenAccountWithExtensions::<TokenAccount>::unpack(account.data.as_slice())
                    .map_err(|err| {
                        tracing::error!(error = ?err, "Failed to deserialize user token account");
                        RestError::TemporarilyUnavailable
                    })
                    .map(|token_account_with_extensions| token_account_with_extensions.base.amount)
            })
            .transpose()?
            .unwrap_or_default();

        Ok(input.amount_user.saturating_sub(native_amount_user) <= amount_user)
    }
}
