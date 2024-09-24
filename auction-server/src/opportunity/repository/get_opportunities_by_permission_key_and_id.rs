use {
    super::{
        Cache,
        Repository,
    },
    crate::{
        kernel::entities::PermissionKey,
        opportunity::entities::opportunity::OpportunityId,
    },
};

impl<T: Cache> Repository<T> {
    pub async fn get_opportunities_by_permission_key_and_id(
        &self,
        id: OpportunityId,
        permission_key: &PermissionKey,
    ) -> Option<T::Opportunity> {
        let opportunities = self.cache.opportunities.read().await;
        opportunities
            .get(permission_key)?
            .iter()
            .find(|o| o.id == id)
            .cloned()
    }
}
