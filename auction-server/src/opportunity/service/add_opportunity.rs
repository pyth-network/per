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
                OpportunityCreate,
            },
            repository::InMemoryStore,
            service::verification::VerifyOpportunityInput,
        },
    },
};

pub struct AddOpportunityInput<T: entities::OpportunityCreate> {
    pub opportunity: T,
}

type OpportunityType<T> = <<T as ChainType>::InMemoryStore as InMemoryStore>::Opportunity;
type OpportunityCreateType<T> = <OpportunityType<T> as entities::Opportunity>::OpportunityCreate;

#[derive(Debug, Clone)]
enum OpportunityAction<T: entities::Opportunity> {
    Add,
    Refresh(T),
    Ignore,
}

impl<T: ChainType> Service<T>
where
    Service<T>: Verification<T>,
{
    async fn assess_action(
        &self,
        opportunity: &OpportunityCreateType<T>,
    ) -> OpportunityAction<OpportunityType<T>> {
        let opportunities = self
            .repo
            .get_in_memory_opportunities_by_key(&opportunity.get_key())
            .await;
        for opp in opportunities.iter() {
            let comparison = opp.compare(opportunity);
            if let entities::OpportunityComparison::Duplicate = comparison {
                return OpportunityAction::Ignore;
            }
            if let entities::OpportunityComparison::NeedsRefresh = comparison {
                return OpportunityAction::Refresh(opp.clone());
            }
        }
        OpportunityAction::Add
    }
    pub async fn add_opportunity(
        &self,
        input: AddOpportunityInput<OpportunityCreateType<T>>,
    ) -> Result<<T::InMemoryStore as InMemoryStore>::Opportunity, RestError> {
        let opportunity_create = input.opportunity;
        let action = self.assess_action(&opportunity_create).await;
        if let OpportunityAction::Ignore = action {
            tracing::info!("Submitted opportunity ignored: {:?}", opportunity_create);
            return Err(RestError::BadParameters(
                "Same opportunity is submitted recently".to_string(),
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

        let opportunity = if let OpportunityAction::Refresh(opp) = action {
            self.repo.refresh_in_memory_opportunity(opp.clone()).await
        } else {
            self.repo
                .add_opportunity(&self.db, opportunity_create.clone())
                .await?
        };

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
