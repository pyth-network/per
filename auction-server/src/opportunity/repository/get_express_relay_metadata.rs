use {
    super::{
        InMemoryStoreSvm,
        Repository,
    },
    express_relay::state::ExpressRelayMetadata,
};

impl Repository<InMemoryStoreSvm> {
    pub async fn query_express_relay_metadata(&self) -> Option<ExpressRelayMetadata> {
        self.in_memory_store
            .express_relay_metadata
            .read()
            .await
            .clone()
    }

    // TODO: This should not be cached forever. Add methods for intelligent cache invalidation.
    pub async fn cache_express_relay_metadata(&self, metadata: ExpressRelayMetadata) {
        *self.in_memory_store.express_relay_metadata.write().await = Some(metadata);
    }
}
