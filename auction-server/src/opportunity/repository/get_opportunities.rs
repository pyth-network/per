use {
    super::{
        Cache,
        Repository,
    },
    crate::kernel::entities::PermissionKey,
    std::collections::HashMap,
};

impl<T: Cache> Repository<T> {
    pub async fn get_opportunities(&self) -> HashMap<PermissionKey, Vec<T::Opportunity>> {
        self.cache.opportunities.read().await.clone()
    }
}
