use {
    super::{
        ChainType,
        Service,
    },
    crate::{
        kernel::entities::PermissionKey,
        opportunity::repository::InMemoryStore,
    },
};

pub struct GetOpportunitiesByPermissionKeyInput {
    pub permission_key: PermissionKey,
}

impl<T: ChainType> Service<T> {
    pub async fn get_live_opportunities_by_permission_key(
        &self,
        input: GetOpportunitiesByPermissionKeyInput,
    ) -> Vec<<T::InMemoryStore as InMemoryStore>::Opportunity> {
        self.repo
            .get_live_opportunities_by_permission_key(input.permission_key)
            .await
    }
}
