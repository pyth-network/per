use {
    super::{
        entities,
        models,
        InMemoryStore,
        OpportunityMetadata,
        OpportunityRemovalReason,
    },
    crate::{
        api::RestError,
        kernel::{
            db::DB,
            entities::{
                ChainId,
                PermissionKey,
            },
        },
        opportunity::entities::Opportunity,
    },
    sqlx::QueryBuilder,
    time::{
        OffsetDateTime,
        PrimitiveDateTime,
    },
};

pub trait OpportunityTable<T: InMemoryStore> {
    async fn add_opportunity(&self, opportunity: &T::Opportunity) -> Result<(), RestError>;
    async fn get_opportunities(
        &self,
        chain_id: ChainId,
        permission_key: Option<PermissionKey>,
        from_time: Option<OffsetDateTime>,
    ) -> Result<Vec<T::Opportunity>, RestError>;
    async fn remove_opportunities(
        &self,
        permission_key: PermissionKey,
        chain_id: ChainId,
        opportunity_key: &entities::OpportunityKey,
        reason: OpportunityRemovalReason,
    ) -> anyhow::Result<()>;
    async fn remove_opportunity(
        &self,
        opportunity: &<T as InMemoryStore>::Opportunity,
        reason: OpportunityRemovalReason,
    ) -> anyhow::Result<()>;
}

impl<T: InMemoryStore> OpportunityTable<T> for DB {
    async fn add_opportunity(&self, opportunity: &T::Opportunity) -> Result<(), RestError> {
        let metadata = opportunity.get_models_metadata();
        let chain_type = <T::Opportunity as entities::Opportunity>::ModelMetadata::get_chain_type();
        sqlx::query!("INSERT INTO opportunity (id,
                                                        creation_time,
                                                        permission_key,
                                                        chain_id,
                                                        chain_type,
                                                        metadata,
                                                        sell_tokens,
                                                        buy_tokens) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        opportunity.id,
        PrimitiveDateTime::new(opportunity.creation_time.date(), opportunity.creation_time.time()),
        opportunity.permission_key.to_vec(),
        opportunity.chain_id,
        chain_type as _,
        serde_json::to_value(metadata).expect("Failed to serialize metadata"),
        serde_json::to_value(&opportunity.sell_tokens).expect("Failed to serialize sell_tokens"),
        serde_json::to_value(&opportunity.buy_tokens).expect("Failed to serialize buy_tokens"))
            .execute(self)
            .await
            .map_err(|e| {
                tracing::error!("DB: Failed to insert opportunity: {}", e);
                RestError::TemporarilyUnavailable
            })?;
        Ok(())
    }

    async fn get_opportunities(
        &self,
        chain_id: ChainId,
        permission_key: Option<PermissionKey>,
        from_time: Option<OffsetDateTime>,
    ) -> Result<Vec<<T as InMemoryStore>::Opportunity>, RestError> {
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
        query.push_bind(super::OPPORTUNITY_PAGE_SIZE_CAP as i64);
        let opps: Vec<models::Opportunity<<T::Opportunity as entities::Opportunity>::ModelMetadata>> = query
            .build_query_as()
            .fetch_all(self)
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

        opps.into_iter().map(|opp| opp.clone().try_into().map_err(
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
        )).collect()
    }

    async fn remove_opportunities(
        &self,
        permission_key: PermissionKey,
        chain_id: ChainId,
        opportunity_key: &entities::OpportunityKey,
        reason: OpportunityRemovalReason,
    ) -> anyhow::Result<()> {
        let now = OffsetDateTime::now_utc();
        sqlx::query("UPDATE opportunity SET removal_time = $1, removal_reason = $2 WHERE permission_key = $3 AND chain_id = $4 and removal_time IS NULL")
            .bind(PrimitiveDateTime::new(now.date(), now.time()))
            .bind(reason)
            .bind(permission_key.as_ref())
            .bind(chain_id)
            .execute(self)
            .await?;
        Ok(())
    }

    async fn remove_opportunity(
        &self,
        opportunity: &<T as InMemoryStore>::Opportunity,
        reason: OpportunityRemovalReason,
    ) -> anyhow::Result<()> {
        let now = OffsetDateTime::now_utc();
        sqlx::query("UPDATE opportunity SET removal_time = $1, removal_reason = $2 WHERE id = $3 AND removal_time IS NULL")
            .bind(PrimitiveDateTime::new(now.date(), now.time()))
            .bind(reason)
            .bind(opportunity.id)
            .execute(self)
            .await?;
        Ok(())
    }
}


// #[cfg(test)]
// mod tests {
//     use crate::opportunity::repository::InMemoryStoreSvm;

//     use super::*;

//     #[tokio::test]
//     async fn test_get_opportunities() {
//         let mut db = MockOpportunityTable::<InMemoryStoreSvm>::default();
//         db.expect_get_opportunities().returning(|_, _, _| Ok(vec![]));
//         let opps = db.get_opportunities("evm".to_string(), None, None).await;
//             println!("{:?}", opps);
//     }
// }
