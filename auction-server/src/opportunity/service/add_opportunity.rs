use {
    super::{
        verification::Verification,
        ChainType,
        Service,
    },
    crate::{
        api::{
            ws::UpdateEvent::NewOpportunity,
            RestError,
        },
        opportunity::{
            entities::{
                self,
                Opportunity,
            },
            repository::InMemoryStore,
            service::verification::VerifyOpportunityInput,
        },
    },
};

pub struct AddOpportunityInput<T: entities::OpportunityCreate> {
    pub opportunity: T,
}

impl<T: ChainType> Service<T>
where
    Service<T>: Verification<T>,
{
    pub async fn add_opportunity(
        &self,
        input: AddOpportunityInput<<<T::InMemoryStore as InMemoryStore>::Opportunity as entities::Opportunity>::OpportunityCreate>,
    ) -> Result<<T::InMemoryStore as InMemoryStore>::Opportunity, RestError> {
        let opportunity_create = input.opportunity;
        if self
            .repo
            .exists_in_memory_opportunity_create(&opportunity_create)
            .await
        {
            tracing::warn!("Duplicate opportunity submission: {:?}", opportunity_create);
            return Err(RestError::BadParameters(
                "Duplicate opportunity submission".to_string(),
            ));
        }

        self.verify_opportunity(VerifyOpportunityInput {
            opportunity: opportunity_create.clone(),
        })
        .await
        .map_err(|e| {
            tracing::warn!(
                "Failed to verify opportunity: {:?} - opportunity: {:?}",
                e,
                opportunity_create,
            );
            e
        })?;

        let opportunity = self
            .repo
            .add_opportunity(&self.db, opportunity_create.clone())
            .await?;

        self.store
            .ws
            .broadcast_sender
            .send(NewOpportunity(opportunity.clone().into()))
            .map_err(|e| {
                tracing::error!(
                    "Failed to send update: {} - opportunity: {:?}",
                    e,
                    opportunity
                );
                RestError::TemporarilyUnavailable
            })?;

        #[allow(clippy::mutable_key_type)]
        let opportunities_map = &self.repo.get_in_memory_opportunities().await;
        tracing::debug!("number of permission keys: {}", opportunities_map.len());
        tracing::debug!(
            "number of opportunities for key: {}",
            opportunities_map
                .get(&opportunity.get_key())
                .map_or(0, |opps| opps.len())
        );

        Ok(opportunity)
    }
}
