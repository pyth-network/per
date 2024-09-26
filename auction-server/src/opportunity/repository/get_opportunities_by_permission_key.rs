use {
    super::{
        models::{
            self,
            OpportunityMetadataEvm,
        },
        InMemoryStoreEvm,
        Repository,
    },
    crate::{
        api::RestError,
        kernel::entities::{
            ChainId,
            PermissionKey,
        },
        opportunity::entities,
    },
    sqlx::QueryBuilder,
    time::OffsetDateTime,
};

impl Repository<InMemoryStoreEvm> {
    pub async fn get_opportunities_by_permission_key(
        &self,
        db: &sqlx::Pool<sqlx::Postgres>,
        chain_id: ChainId,
        permission_key: Option<PermissionKey>,
        from_time: Option<OffsetDateTime>,
    ) -> Result<Vec<entities::OpportunityEvm>, RestError> {
        let mut query = QueryBuilder::new("SELECT * from opportunity where chain_id = ");
        query.push_bind(chain_id.clone());
        if let Some(permission_key) = permission_key.clone() {
            query.push(" AND permission_key = ");
            query.push_bind(permission_key.to_vec());
        }
        if let Some(from_time) = from_time {
            query.push(" AND creation_time >= ");
            query.push_bind(from_time);
        }
        query.push(" ORDER BY creation_time ASC LIMIT 20");
        let opps: Vec<models::Opportunity<OpportunityMetadataEvm>> = query
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
        let parsed_opps: anyhow::Result<Vec<entities::OpportunityEvm>> =
            opps.into_iter().map(|opp| opp.try_into()).collect();
        parsed_opps.map_err(|e| {
            tracing::error!(
                "Failed to convert opportunity to OpportunityParamsWithMetadata: {} - chain_id: {:?} - permission_key: {:?} - from_time: {:?}",
                e,
                chain_id,
                permission_key,
                from_time,
            );
            RestError::TemporarilyUnavailable
        })
    }
}
