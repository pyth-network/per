use {
    super::{
        InMemoryStore,
        Repository,
    },
    crate::kernel::entities::PermissionKey,
};

impl<T: InMemoryStore> Repository<T> {
    pub async fn get_live_opportunities_by_permission_key(
        &self,
        permission_key: PermissionKey,
    ) -> Vec<T::Opportunity> {
        self.in_memory_store
            .opportunities
            .read()
            .await
            .get(&permission_key)
            .map(|opportunities| opportunities.clone())
            .unwrap_or_default()
    }
}
