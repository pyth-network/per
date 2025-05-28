use {
    super::{
        add_opportunity_analytics::AddOpportunityAnalyticsInput,
        Service,
    },
    crate::{
        api::RestError,
        opportunity::entities,
    },
};

pub struct RemoveOpportunityInput {
    pub opportunity: entities::OpportunitySvm,
    pub reason:      entities::OpportunityRemovalReason,
}

impl Service {
    pub async fn remove_opportunity(&self, input: RemoveOpportunityInput) -> Result<(), RestError> {
        self.get_config(&input.opportunity.chain_id)?;
        if let Some(removal_time) = self
            .repo
            .remove_opportunity(&input.opportunity, input.reason.clone())
            .await
            .map_err(|e| {
                tracing::error!(
                    error = ?e,
                    opportunity = ?input.opportunity,
                    "Failed to remove opportunity",
                );
                RestError::TemporarilyUnavailable
            })?
        {
            self.task_tracker.spawn({
                let service = self.clone();
                async move {
                    service
                        .add_opportunity_analytics(AddOpportunityAnalyticsInput {
                            opportunity:    input.opportunity,
                            removal_time:   Some(removal_time),
                            removal_reason: Some(input.reason),
                        })
                        .await
                }
            });
        }
        Ok(())
    }
}
