use {
    super::Service,
    crate::{
        auction::entities::BidStatus,
        kernel::entities::PermissionKeySvm,
    },
};

impl Service {
    pub async fn get_permission_keys_for_auction(&self) -> Vec<PermissionKeySvm> {
        let pending_bids = self.repo.get_in_memory_pending_bids().await;
        pending_bids
            .iter()
            .filter(|(_, bids)| bids.iter().any(|bid| bid.status.is_pending()))
            .map(|(key, _)| key.clone())
            .collect()
    }
}
