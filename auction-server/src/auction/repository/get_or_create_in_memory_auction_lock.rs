use {
    super::Repository,
    crate::auction::{
        entities,
        service::ChainTrait,
    },
};

impl<T: ChainTrait> Repository<T> {
    pub async fn get_or_create_in_memory_auction_lock(
        &self,
        key: entities::PermissionKey<T>,
    ) -> entities::AuctionLock {
        self.in_memory_store
            .auction_lock
            .lock()
            .await
            .entry(key)
            .or_default()
            .clone()
    }
}
