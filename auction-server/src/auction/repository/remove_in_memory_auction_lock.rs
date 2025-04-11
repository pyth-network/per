use {
    super::Repository,
    crate::kernel::entities::PermissionKeySvm,
    std::sync::Arc,
};

impl Repository {
    pub async fn remove_in_memory_auction_lock(&self, key: &PermissionKeySvm) {
        let mut mutex_guard = self.in_memory_store.auction_lock.lock().await;
        let auction_lock = mutex_guard.get(key);
        if let Some(auction_lock) = auction_lock {
            // Whenever there is no thread borrowing a lock for this key, we can remove it from the locks HashMap.
            if Arc::strong_count(auction_lock) == 1 {
                mutex_guard.remove(key);
            }
        }
    }
}
