use {
    super::Repository,
    crate::{
        auction::entities,
        kernel::entities::PermissionKeySvm,
    },
    std::collections::HashMap,
};

impl Repository {
    pub async fn get_in_memory_pending_bids(
        &self,
    ) -> HashMap<PermissionKeySvm, Vec<entities::Bid>> {
        self.in_memory_store.pending_bids.read().await.clone()
    }
}
