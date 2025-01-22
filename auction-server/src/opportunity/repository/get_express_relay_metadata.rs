use {
    super::{
        InMemoryStoreSvm,
        Repository,
    },
    express_relay::state::ExpressRelayMetadata,
    std::time::Duration,
    tokio::time::Instant,
};
const MAX_METADATA_STALENESS: Duration = Duration::from_secs(3600);

impl Repository<InMemoryStoreSvm> {
    pub async fn query_express_relay_metadata(&self) -> Option<ExpressRelayMetadata> {
        let (insert_time, metadata) = self
            .in_memory_store
            .express_relay_metadata
            .read()
            .await
            .clone()?;
        if insert_time.elapsed() > MAX_METADATA_STALENESS {
            None
        } else {
            Some(metadata)
        }
    }

    pub async fn cache_express_relay_metadata(&self, metadata: ExpressRelayMetadata) {
        *self.in_memory_store.express_relay_metadata.write().await =
            Some((Instant::now(), metadata));
    }
}
