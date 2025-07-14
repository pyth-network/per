use {
    super::Repository,
    crate::auction::entities,
};

impl Repository {
    #[tracing::instrument(skip_all)]
    pub async fn get_or_create_in_memory_bid_lock(
        &self,
        key: entities::BidId,
    ) -> entities::BidLock {
        self.in_memory_store
            .bid_lock
            .lock()
            .await
            .entry(key)
            .or_default()
            .clone()
    }
}
