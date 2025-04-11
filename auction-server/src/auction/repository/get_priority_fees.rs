use {
    super::{
        PrioritizationFeeSample,
        Repository,
    },
    time::OffsetDateTime,
};

impl Repository {
    pub async fn get_priority_fees(&self, after: OffsetDateTime) -> Vec<PrioritizationFeeSample> {
        self.in_memory_store
            .chain_store
            .recent_prioritization_fees
            .read()
            .await
            .iter()
            .filter(|sample| sample.sample_time > after)
            .cloned()
            .collect()
    }
}
