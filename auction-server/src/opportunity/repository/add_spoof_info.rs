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
    ethers::types::{
        spoof,
        Address,
    },
};

impl Repository<CacheEvm> {
    pub async fn add_spoof_info(&self, spoof_info: SpoofInfo) {
        self.cache
            .spoof_info
            .write()
            .await
            .insert(spoof_info.token, spoof_info.state);
    }
}
