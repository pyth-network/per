use {
    super::{
        CacheEvm,
        Repository,
    },
    crate::opportunity::entities,
};

impl Repository<CacheEvm> {
    pub async fn add_spoof_info(&self, spoof_info: entities::SpoofInfo) {
        self.cache
            .spoof_info
            .write()
            .await
            .insert(spoof_info.token, spoof_info.state);
    }
}
