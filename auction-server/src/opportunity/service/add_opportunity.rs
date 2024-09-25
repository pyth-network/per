use {
    super::{
        verify_opportunity::VerifyOpportunityInput,
        ChainTypeEvm,
        Service,
    },
    crate::{
        api::{
            ws::UpdateEvent::NewOpportunity,
            RestError,
        },
        opportunity::{
            api::OpportunityParamsWithMetadata,
            entities,
        },
    },
};

pub struct AddOpportunityInput {
    pub opportunity: entities::OpportunityEvm,
}

impl Service<ChainTypeEvm> {
    pub async fn add_opportunity(
        &self,
        input: AddOpportunityInput,
    ) -> Result<entities::OpportunityEvm, RestError> {
        let opportunity = input.opportunity;
        self.verify_opportunity(VerifyOpportunityInput {
            opportunity: opportunity.clone(),
        })
        .await
        .map_err(|e| {
            tracing::warn!(
                "Failed to verify opportunity: {:?} - opportunity: {:?}",
                e,
                opportunity,
            );
            e
        })?;

        if self.repo.opportunity_exists(&opportunity).await {
            tracing::warn!("Duplicate opportunity submission: {:?}", opportunity);
            return Err(RestError::BadParameters(
                "Duplicate opportunity submission".to_string(),
            ));
        }
        self.repo
            .add_opportunity(&self.db, opportunity.clone())
            .await?;

        self.store
            .ws
            .broadcast_sender
            .send(NewOpportunity(OpportunityParamsWithMetadata::from(
                opportunity.clone(),
            )))
            .map_err(|e| {
                tracing::error!(
                    "Failed to send update: {} - opportunity: {:?}",
                    e,
                    opportunity
                );
                RestError::TemporarilyUnavailable
            })?;

        let opportunities_map = &self.repo.get_opportunities().await;
        tracing::debug!("number of permission keys: {}", opportunities_map.len());
        tracing::debug!(
            "number of opportunities for key: {}",
            opportunities_map
                .get(&opportunity.permission_key)
                .map_or(0, |opps| opps.len())
        );

        Ok(opportunity)
    }
}
