use {
    super::{
        InMemoryStoreSvm,
        Repository,
    },
    solana_sdk::pubkey::Pubkey,
};

impl Repository<InMemoryStoreSvm> {
    pub async fn query_token_program_cache(&self, mint: Pubkey) -> Option<Pubkey> {
        let cache_read = self.in_memory_store.token_program_cache.read().await;
        let token_program_query = cache_read.get(&mint);
        token_program_query.cloned()
    }

    pub async fn cache_token_program(&self, mint: Pubkey, token_program: Pubkey) {
        self.in_memory_store
            .token_program_cache
            .write()
            .await
            .insert(mint, token_program);
    }
}
