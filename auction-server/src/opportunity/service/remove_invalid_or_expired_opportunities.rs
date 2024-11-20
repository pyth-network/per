use {
    super::{
        verification::{
            Verification,
            VerifyOpportunityInput,
        },
        ChainType,
        Service,
    },
    crate::{
        api::{
            ws::UpdateEvent,
            RestError,
        },
        opportunity::{
            entities::{
                self,
                Opportunity as _,
            },
            service::ChainTypeEnum,
        },
    },
    time::{
        Duration,
        OffsetDateTime,
    },
};

const MAX_STALE_OPPORTUNITY_DURATION: Duration = Duration::seconds(60);

impl<T: ChainType> Service<T>
where
    Service<T>: Verification<T>,
{
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
                    _ => None,
                };

                if let Some(reason) = reason {
                    tracing::info!(
                        opportunity = ?opportunity,
                        reason = ?reason,
                        "Removing Opportunity",
                    );
                    match self
                        .repo
                        .remove_opportunity(&self.db, opportunity, reason)
                        .await
                    {
                        Ok(()) => {
                            // TODO Remove this later
                            // For now we don't want searchers to update any of their code on EVM chains.
                            // So we are not broadcasting remove opportunities event for EVM chains.
                            if T::get_type() == ChainTypeEnum::Evm {
                                continue;
                            }

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
