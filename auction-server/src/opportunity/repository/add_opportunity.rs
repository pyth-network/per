use {
    super::{
        db::OpportunityTable,
        models::OpportunityMetadata,
        InMemoryStore,
        Repository,
    },
    crate::{
        api::RestError,
        opportunity::entities::{
            self,
            Opportunity,
        },
    },
    sqlx::Postgres,
    time::PrimitiveDateTime,
};

impl<T: InMemoryStore> Repository<T> {
    pub async fn add_opportunity(
        &self,
        db: &sqlx::Pool<Postgres>,
        opportunity: <T::Opportunity as entities::Opportunity>::OpportunityCreate,
    ) -> Result<T::Opportunity, RestError> {
        let opportunity: T::Opportunity = T::Opportunity::new_with_current_time(opportunity);
        OpportunityTable::<T>::add_opportunity(db, &opportunity).await?;
        self.in_memory_store
            .opportunities
            .write()
            .await
            .entry(opportunity.get_key())
            .or_insert_with(Vec::new)
            .push(opportunity.clone());

        Ok(opportunity)
    }
}
