use {
    super::Service,
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
                let (service, opportunity) = (self.clone(), input.opportunity.clone());
                async move {
                    service
                        .repo
                        .add_opportunity_analytics(
                            opportunity.clone(),
                            Some(removal_time),
                            Some(input.reason),
                        )
                        .await
                        .map_err(|err| {
                            tracing::error!(
                                error = ?err,
                                opportunity = ?opportunity,
                                "Failed to add opportunity analytics",
                            );
                        })
                }
            });
        }
        Ok(())
    }
}
