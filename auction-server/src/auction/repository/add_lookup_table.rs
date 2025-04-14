use {
    super::Repository,
    solana_sdk::pubkey::Pubkey,
};

impl Repository {
    pub async fn add_lookup_table(&self, key: Pubkey, addresses: Vec<Pubkey>) {
        self.in_memory_store
            .chain_store
            .lookup_table
            .write()
            .await
            .insert(key, addresses);
    }
}
