use {
    super::{
        InMemoryStore,
        Repository,
    },
    crate::{
        kernel::entities::PermissionKey,
        opportunity::entities,
    },
};

impl<T: InMemoryStore> Repository<T> {
    pub async fn get_opportunities_by_permission_key_and_id(
        &self,
        id: entities::OpportunityId,
        permission_key: &PermissionKey,
    ) -> Option<T::Opportunity> {
        let opportunities = self.in_memory_store.opportunities.read().await;
        opportunities
            .get(permission_key)?
            .iter()
            .find(|o| o.id == id)
            .cloned()
    }
}
