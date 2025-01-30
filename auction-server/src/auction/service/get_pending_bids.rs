use {
    super::{
        ChainTrait,
        Service,
    },
    crate::auction::entities,
};

pub struct GetLiveBidsInput<T: entities::BidChainData> {
    pub permission_key: T::PermissionKey,
}

impl<T: ChainTrait> Service<T> {
    pub async fn get_pending_bids(
        &self,
        input: GetLiveBidsInput<T::BidChainDataType>,
    ) -> Vec<entities::Bid<T>> {
        self.repo
            .get_in_memory_pending_bids_by_permission_key(&input.permission_key)
            .await
    }
}
