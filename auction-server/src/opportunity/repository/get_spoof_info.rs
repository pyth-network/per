use {
    super::{
        InMemoryStoreEvm,
        Repository,
    },
    crate::opportunity::{
        entities,
        service::{
            ChainTypeEvm,
            ChainTypeSvm,
        },
    },
    ethers::types::Address,
};

impl Repository<InMemoryStoreEvm> {
    pub async fn get_spoof_info(&self, token: Address) -> Option<entities::SpoofInfo> {
        let state = self
            .in_memory_store
            .spoof_info
            .read()
            .await
            .get(&token)
            .cloned();
        state.map(|state| entities::SpoofInfo { token, state })
    }
}
