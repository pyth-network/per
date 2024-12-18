use {
    super::Repository,
    crate::kernel::entities::Svm,
    solana_sdk::pubkey::Pubkey,
};

impl Repository<Svm> {
    pub async fn get_lookup_table(&self, key: &Pubkey) -> Option<Vec<Pubkey>> {
        self.in_memory_store
            .chain_store
            .lookup_table
            .read()
            .await
            .get(key)
            .cloned()
    }
}
