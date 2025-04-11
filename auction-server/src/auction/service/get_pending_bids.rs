use {
    super::Service,
    crate::{
        auction::entities,
        kernel::entities::PermissionKeySvm,
    },
};

pub struct GetLiveBidsInput {
    pub permission_key: PermissionKeySvm,
}

impl Service {
    pub async fn get_pending_bids(&self, input: GetLiveBidsInput) -> Vec<entities::Bid> {
        self.repo
            .get_in_memory_pending_bids_by_permission_key(&input.permission_key)
            .await
    }
}
