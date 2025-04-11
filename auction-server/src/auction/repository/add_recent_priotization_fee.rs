use {
    super::Repository,
    crate::auction::{
        repository::PrioritizationFeeSample,
        service::update_recent_prioritization_fee::RECENT_FEES_SLOT_WINDOW,
    },
    time::OffsetDateTime,
};

impl Repository {
    pub async fn add_recent_prioritization_fee(&self, fee: u64) {
        let mut write_guard = self
            .in_memory_store
            .chain_store
            .recent_prioritization_fees
            .write()
            .await;
        let sample = PrioritizationFeeSample {
            fee,
            sample_time: OffsetDateTime::now_utc(),
        };
        write_guard.push_back(sample.clone());
        if write_guard.len() > RECENT_FEES_SLOT_WINDOW {
            write_guard.pop_front();
        }
        tracing::info!("Last prioritization fee: {:?}", sample);
    }
}
