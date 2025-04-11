use {
    super::Repository,
    crate::{
        auction::entities,
        kernel::entities::PermissionKeySvm,
    },
};

impl Repository {
    pub async fn get_in_memory_pending_bids_by_permission_key(
        &self,
        permission_key: &PermissionKeySvm,
    ) -> Vec<entities::Bid> {
        self.in_memory_store
            .pending_bids
            .read()
            .await
            .get(permission_key)
            .cloned()
            .unwrap_or_default()
    }
}
