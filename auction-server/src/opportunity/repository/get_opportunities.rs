use {
    super::{
        InMemoryStore,
        Repository,
    },
    crate::kernel::entities::PermissionKey,
    std::collections::HashMap,
};

impl<T: InMemoryStore> Repository<T> {
    pub async fn get_opportunities(&self) -> HashMap<PermissionKey, Vec<T::Opportunity>> {
        self.in_memory_store.opportunities.read().await.clone()
    }
}
