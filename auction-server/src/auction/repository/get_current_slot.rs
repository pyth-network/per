use {
    super::{
        Repository,
    },
    crate::kernel::entities::Svm,
};

impl Repository<Svm> {
    pub async fn get_current_slot(&self) -> u64 {
        *self.in_memory_store
            .chain_store
            .current_slot
            .read()
            .await
        }
}
