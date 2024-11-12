use {
    super::{
        Service,
        ServiceTrait,
    },
    crate::bid::entities,
};

pub struct GetLiveBidsInput<T: entities::BidChainData> {
    pub permission_key: T::PermissionKey,
}

impl<T: ServiceTrait> Service<T> {
    pub async fn get_live_bids(
        &self,
        input: GetLiveBidsInput<T::ChainData>,
    ) -> Vec<entities::Bid<T>> {
        self.repo
            .get_in_memory_bids_by_permission_key(&input.permission_key)
            .await
    }
}
