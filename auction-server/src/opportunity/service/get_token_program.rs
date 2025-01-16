use {
    super::{
        ChainTypeSvm,
        Service,
    },
    crate::{
        api::RestError,
        kernel::entities::ChainId,
    },
    solana_sdk::pubkey::Pubkey,
};

pub struct GetTokenProgramInput {
    pub chain_id: ChainId,
    pub mint:     Pubkey,
}

impl Service<ChainTypeSvm> {
    /// Find the token program for a given mint.
    /// Pulls from the cache if already present, otherwise queries the RPC and saves in the cache.
    pub async fn get_token_program(
        &self,
        input: GetTokenProgramInput,
    ) -> Result<Pubkey, RestError> {
        let config = self.get_config(&input.chain_id)?;
        let cache_read = self.repo.in_memory_store.token_program_cache.read().await;
        let token_program_query = cache_read.get(&input.mint);
        let token_program = match token_program_query {
            Some(program) => *program,
            None => {
                // need to drop the cache in order to be able to get write access
                drop(cache_read);
                let token_program_address = config
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
                    })?
                    .owner;
                self.repo
                    .in_memory_store
                    .token_program_cache
                    .write()
                    .await
                    .insert(input.mint, token_program_address);
                token_program_address
            }
        };

        if !config.accepted_token_programs.contains(&token_program) {
            tracing::error!(
                "Token program {program} for mint account {mint} is not an approved token program",
                program = token_program,
                mint = input.mint
            );
            return Err(RestError::BadParameters(format!(
                "Provided mint belongs to unapproved token program {}",
                token_program
            )));
        }
        Ok(token_program)
    }
}
