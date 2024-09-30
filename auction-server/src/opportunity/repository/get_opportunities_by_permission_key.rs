use {
    super::{
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
    pub async fn get_opportunities_by_permission_key(
        &self,
        db: &sqlx::Pool<sqlx::Postgres>,
        chain_id: ChainId,
        permission_key: Option<PermissionKey>,
        from_time: Option<OffsetDateTime>,
    ) -> Result<Vec<T::Opportunity>, RestError> {
        let mut query = QueryBuilder::new("SELECT * from opportunity WHERE chain_type = ");
        query.push_bind(
            <<T::Opportunity as entities::Opportunity>::ModelMetadata>::get_chain_type(),
        );
        query.push(" AND chain_id = ");
        query.push_bind(chain_id.clone());
        if let Some(permission_key) = permission_key.clone() {
            query.push(" AND permission_key = ");
            query.push_bind(permission_key.to_vec());
        }
        if let Some(from_time) = from_time {
            query.push(" AND creation_time >= ");
            query.push_bind(from_time);
        }
        query.push(" ORDER BY creation_time ASC LIMIT ");
        query.push_bind(super::OPPORTUNITY_PAGE_SIZE);
        let opps: Vec<models::Opportunity<<T::Opportunity as entities::Opportunity>::ModelMetadata>> = query
            .build_query_as()
            .fetch_all(db)
            .await
            .map_err(|e| {
                tracing::error!(
                    "DB: Failed to fetch opportunities: {} - chain_id: {:?} - permission_key: {:?} - from_time: {:?}",
                    e,
                    chain_id,
                    permission_key,
                    from_time,
                );
                RestError::TemporarilyUnavailable
            })?;

        let opportunities: Result<Vec<T::Opportunity>, RestError> = opps.clone().into_iter().map(|opp| opp.clone().try_into().map_err(
            |_| {
                tracing::error!(
                    "Failed to convert database opportunity to entity opportunity: {:?} - chain_id: {:?} - permission_key: {:?} - from_time: {:?}",
                    opp,
                    chain_id,
                    permission_key,
                    from_time,
                );
                RestError::TemporarilyUnavailable
            }
        )).collect();
        opportunities
    }
}
