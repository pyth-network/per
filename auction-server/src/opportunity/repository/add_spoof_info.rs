use {
    super::{
        InMemoryStoreEvm,
        Repository,
    },
    crate::opportunity::entities,
};

impl Repository<InMemoryStoreEvm> {
    pub async fn add_spoof_info(&self, spoof_info: entities::SpoofInfo) {
        self.in_memory_store
            .spoof_info
            .write()
            .await
            .insert(spoof_info.token, spoof_info.state);
    }
}
