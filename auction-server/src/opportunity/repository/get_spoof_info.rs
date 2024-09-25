use {
    super::{
        CacheEvm,
        Repository,
    },
    crate::opportunity::entities,
    ethers::types::Address,
};

impl Repository<CacheEvm> {
    pub async fn get_spoof_info(&self, token: Address) -> Option<entities::SpoofInfo> {
        let state = self.cache.spoof_info.read().await.get(&token).cloned();
        state.map(|state| entities::SpoofInfo { token, state })
    }
}
