use {
    super::Repository,
    crate::{
        auction::entities,
        kernel::entities::PermissionKeySvm,
    },
};

impl Repository {
    pub async fn get_or_create_in_memory_auction_lock(
        &self,
        key: PermissionKeySvm,
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
