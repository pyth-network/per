use {
    super::Repository,
    crate::auction::{
        entities,
        service::ChainTrait,
    },
    std::collections::HashMap,
};

impl<T: ChainTrait> Repository<T> {
    pub async fn get_in_memory_bids(
        &self,
    ) -> HashMap<entities::PermissionKey<T>, Vec<entities::Bid<T>>> {
        self.in_memory_store.bids.read().await.clone()
    }
}
