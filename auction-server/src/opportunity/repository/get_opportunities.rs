use {
    super::{
        db::OpportunityTable,
        models::{
            self,
        },
        InMemoryStore,
        Repository,
    },
    crate::{
        api::RestError,
        kernel::entities::{
            ChainId,
            PermissionKey,
        },
        opportunity::{
            entities,
            repository::models::OpportunityMetadata,
        },
    },
    sqlx::QueryBuilder,
    time::OffsetDateTime,
};

impl<T: InMemoryStore> Repository<T> {
    pub async fn get_opportunities(
        &self,
        db: &sqlx::Pool<sqlx::Postgres>,
        chain_id: ChainId,
        permission_key: Option<PermissionKey>,
        from_time: Option<OffsetDateTime>,
    ) -> Result<Vec<T::Opportunity>, RestError> {
        OpportunityTable::<T>::get_opportunities(db, chain_id, permission_key, from_time).await
    }
}
