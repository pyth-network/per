use {
    super::Repository,
    crate::kernel::entities::Svm,
};

impl Repository<Svm> {
    pub async fn add_current_slot(&self, slot: u64) {
        let mut write_guard = self
            .in_memory_store
            .chain_store
            .current_slot
            .write()
            .await;
        *write_guard = slot;
    }
}
