use {
    super::Repository,
    crate::{
        kernel::entities::PermissionKey,
        opportunity::entities::opportunity::{
            Opportunity,
            OpportunityId,
        },
    },
};

impl<T: Opportunity> Repository<T> {
    pub async fn get_opportunities_by_permission_key_and_id(
        &self,
        id: OpportunityId,
        permission_key: &PermissionKey,
    ) -> Option<T> {
        let opportunities = self.opportunities.read().await;
        opportunities
            .get(permission_key)?
            .iter()
            .find(|o| o.id == id)
            .cloned()
    }
}
