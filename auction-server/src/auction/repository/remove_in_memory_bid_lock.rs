use {
    super::Repository,
    crate::auction::{
        entities,
        service::ChainTrait,
    },
    std::sync::Arc,
};

impl<T: ChainTrait> Repository<T> {
    pub async fn remove_in_memory_bid_lock(&self, key: &entities::BidId) {
        let mut mutex_guard = self.in_memory_store.bid_lock.lock().await;
        let bid_lock = mutex_guard.get(key);
        if let Some(bid_lock) = bid_lock {
            // Whenever there is no thread borrowing a lock for this key, we can remove it from the locks HashMap.
            if Arc::strong_count(bid_lock) == 1 {
                mutex_guard.remove(key);
            }
        }
    }
}
