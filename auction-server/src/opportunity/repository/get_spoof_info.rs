use {
    super::{
        CacheEvm,
        Repository,
    },
    crate::{
        kernel::entities::PermissionKey,
        opportunity::{
            entities::{
                opportunity::{
                    Opportunity,
                    OpportunityId,
                },
                opportunity_evm::OpportunityEvm,
                spoof_info::SpoofInfo,
            },
            token_spoof,
        },
    },
    ethers::types::Address,
};

impl Repository<CacheEvm> {
    pub async fn get_spoof_info(&self, token: Address) -> Option<SpoofInfo> {
        let state = self.cache.spoof_info.read().await.get(&token).cloned();
        state.map(|state| SpoofInfo { token, state })
    }
}
