use {
    super::Service,
    crate::{
        api::RestError,
        kernel::entities::ChainId,
        opportunity::entities,
    },
    solana_sdk::{
        program_pack::Pack,
        pubkey::Pubkey,
    },
    spl_token::state::Mint,
    spl_token_2022::{
        extension::StateWithExtensionsOwned,
        state::Mint as Mint2022,
    },
};

pub struct GetTokenMintInput {
    pub chain_id: ChainId,
    pub mint:     Pubkey,
}

impl Service {
    /// Find the token mint data for a given mint.
    /// Pulls from the cache if already present, otherwise queries the RPC and saves in the cache.
    pub async fn get_token_mint(
        &self,
        input: GetTokenMintInput,
    ) -> Result<entities::TokenMint, RestError> {
        let config = self.get_config(&input.chain_id)?;
        let token_mint = match self.repo.query_token_mint_cache(input.mint).await {
            Some(data) => data,
            None => {
                let account = config
                    .rpc_client
                    .get_account(&input.mint)
                    .await
                    .map_err(|err| {
                        tracing::error!(
                            "Failed to retrieve owner program for mint account {mint}: {:?}",
                            err,
                            mint = input.mint
                        );
                        RestError::BadParameters(format!(
                            "Failed to retrieve owner program for mint account {}: {:?}",
                            input.mint, err
                        ))
                    })?;
                let owner = account.owner;
                let decimals = if owner == spl_token::id() {
                    Mint::unpack(&account.data)
                        .map_err(|err| {
                            tracing::error!(
                                mint = ?input.mint,
                                error = ?err,
                                "Failed to unpack mint account",
                            );
                            RestError::TemporarilyUnavailable
                        })?
                        .decimals
                } else {
                    StateWithExtensionsOwned::<Mint2022>::unpack(account.data)
                        .map_err(|err| {
                            tracing::error!(
                                mint = ?input.mint,
                                error = ?err,
                                "Failed to unpack mint account token 2022",
                            );
                            RestError::TemporarilyUnavailable
                        })?
                        .base
                        .decimals
                };
                let token_mint = entities::TokenMint {
                    mint: input.mint,
                    decimals,
                    token_program: owner,
                };
                self.repo
                    .cache_token_mint(input.mint, token_mint.clone())
                    .await;
                token_mint
            }
        };
        if !config
            .accepted_token_programs
            .contains(&token_mint.token_program)
        {
            tracing::error!(
                program = ?token_mint.token_program,
                mint = ?input.mint,
                "Token program for mint account is not an approved token program",
            );
            return Err(RestError::BadParameters(format!(
                "Provided mint belongs to unapproved token program {}",
                token_mint.token_program
            )));
        }
        Ok(token_mint)
    }
}
