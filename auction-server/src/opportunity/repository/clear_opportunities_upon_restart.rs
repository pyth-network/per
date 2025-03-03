use {
    super::{
        db::OpportunityTable,
        InMemoryStore,
        Repository,
    },
    crate::api::RestError,
};

impl<T: InMemoryStore, U: OpportunityTable<T>> Repository<T, U> {
    pub async fn clear_opportunities_upon_restart(&self) -> Result<(), RestError> {
        self.db.clear_opportunities_upon_restart().await
    }
}
