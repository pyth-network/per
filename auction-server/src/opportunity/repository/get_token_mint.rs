use {
    super::Repository,
    crate::opportunity::entities,
    solana_sdk::pubkey::Pubkey,
};

impl Repository {
    pub async fn query_token_mint_cache(&self, mint: Pubkey) -> Option<entities::TokenMint> {
        let cache_read = self.in_memory_store.token_mint_cache.read().await;
        let token_mint_query = cache_read.get(&mint);
        token_mint_query.cloned()
    }

    pub async fn cache_token_mint(&self, mint: Pubkey, token_mint: entities::TokenMint) {
        self.in_memory_store
            .token_mint_cache
            .write()
            .await
            .insert(mint, token_mint);
    }
}
