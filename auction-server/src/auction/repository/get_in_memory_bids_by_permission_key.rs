use {
    super::Repository,
    crate::auction::{
        entities,
        service::ChainTrait,
    },
};

impl<T: ChainTrait> Repository<T> {
    pub async fn get_in_memory_bids_by_permission_key(
        &self,
        permission_key: &<T::BidChainDataType as entities::BidChainData>::PermissionKey,
    ) -> Vec<entities::Bid<T>> {
        self.in_memory_store
            .bids
            .read()
            .await
            .get(permission_key)
            .cloned()
            .unwrap_or_default()
    }
}
