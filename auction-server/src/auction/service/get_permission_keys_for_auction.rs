use {
    super::{
        ChainTrait,
        Service,
    },
    crate::auction::entities::{
        self,
        BidStatus,
    },
};

impl<T: ChainTrait> Service<T> {
    pub async fn get_permission_keys_for_auction(&self) -> Vec<entities::PermissionKey<T>> {
        let live_bids = self.repo.get_in_memory_bids().await;
        live_bids
            .iter()
            .filter(|(_, bids)| bids.iter().any(|bid| bid.status.is_pending()))
            .map(|(key, _)| key.clone())
            .collect()
    }
}
