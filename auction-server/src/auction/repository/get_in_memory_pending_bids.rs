use {
    super::Repository,
    crate::auction::{
        entities,
        service::ChainTrait,
    },
    std::collections::HashMap,
};

impl<T: ChainTrait> Repository<T> {
    pub async fn get_in_memory_pending_bids(
        &self,
    ) -> HashMap<entities::PermissionKey<T>, Vec<entities::Bid<T>>> {
        self.in_memory_store.pending_bids.read().await.clone()
    }
}
