use {
    super::Repository,
    crate::{
        auction::{
            repository::PrioritizationFeeSample,
            service::update_recent_prioritization_fee::RECENT_FEES_SLOT_WINDOW,
        },
        kernel::entities::Svm,
    },
    time::OffsetDateTime,
};

impl Repository<Svm> {
    pub async fn add_recent_priotization_fee(&self, fee: u64) {
        let mut write_guard = self
            .in_memory_store
            .chain_store
            .recent_prioritization_fees
            .write()
            .await;
        write_guard.push_back(PrioritizationFeeSample {
            fee,
            sample_time: OffsetDateTime::now_utc(),
        });
        if write_guard.len() > RECENT_FEES_SLOT_WINDOW {
            write_guard.pop_front();
        }
        tracing::info!("Recent prioritization fees: {:?}", write_guard);
    }
}
