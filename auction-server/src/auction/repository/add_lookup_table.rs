use {
    super::Repository,
    crate::kernel::entities::Svm,
    solana_sdk::pubkey::Pubkey,
};

impl Repository<Svm> {
    pub async fn add_lookup_table(&self, key: Pubkey, addresses: Vec<Pubkey>) {
        self.in_memory_store
            .chain_store
            .lookup_table
            .write()
            .await
            .insert(key, addresses);
    }
}
