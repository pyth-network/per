use {
    super::{
        verification::VerifyOpportunityInput,
        Service,
    },
    crate::{
        api::{
            ws::UpdateEvent,
            RestError,
        },
        opportunity::entities::{
            self,
        },
    },
    time::{
        Duration,
        OffsetDateTime,
    },
};

const MAX_STALE_OPPORTUNITY_DURATION: Duration = Duration::minutes(2);

impl Service {
    pub async fn remove_invalid_or_expired_opportunities(&self) {
        let all_opportunities = self.repo.get_in_memory_opportunities().await;
        for (_, opportunities) in all_opportunities.iter() {
            // check each of the opportunities for this permission key for validity
            for opportunity in opportunities.iter() {
                let reason = match self
                    .verify_opportunity(VerifyOpportunityInput {
                        opportunity: opportunity.clone().into(),
                    })
                    .await
                {
                    Ok(entities::OpportunityVerificationResult::UnableToSpoof) => {
                        if OffsetDateTime::now_utc() - opportunity.refresh_time
                            > MAX_STALE_OPPORTUNITY_DURATION
                        {
                            Some(entities::OpportunityRemovalReason::Expired)
                        } else {
                            None
                        }
                    }
                    Err(e) => {
                        if let RestError::InvalidOpportunity(_) = e {
                            Some(entities::OpportunityRemovalReason::Invalid(e))
                        } else {
                            None
                        }
                    }
                };

                if let Some(reason) = reason {
                    tracing::info!(
                        opportunity = ?opportunity,
                        reason = ?reason,
                        "Removing Opportunity",
                    );

                    match self.repo.remove_opportunity(opportunity, reason).await {
                        Ok(()) => {
                            // If there are no more opportunities with this key, it means all of the
                            // opportunities have been removed for this key, so we can broadcast remove opportunities event.
                            if self
                                .repo
                                .get_in_memory_opportunities_by_key(&opportunity.get_key())
                                .await
                                .is_empty()
                            {
                                if let Err(e) = self.store.ws.broadcast_sender.send(
                                    UpdateEvent::RemoveOpportunities(
                                        opportunity.get_opportunity_delete(),
                                    ),
                                ) {
                                    tracing::error!(
                                        error = e.to_string(),
                                        "Failed to broadcast remove opportunity"
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!(error = ?e, "Failed to remove opportunity");
                        }
                    }
                }
            }
        }
    }
}
