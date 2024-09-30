use {
    super::{
        verification::{
            Verification,
            VerifyOpportunityInput,
        },
        ChainTypeEvm,
        Service,
    },
    crate::{
        api::RestError,
        opportunity::entities,
        state::UnixTimestampMicros,
    },
    std::time::{
        SystemTime,
        UNIX_EPOCH,
    },
};

const MAX_STALE_OPPORTUNITY_MICROS: i128 = 60_000_000;

impl Service<ChainTypeEvm>
where
    Service<ChainTypeEvm>: Verification<ChainTypeEvm>,
{
    pub async fn remove_invalid_or_expired_opportunities(&self) {
        let all_opportunities = self.repo.get_opportunities().await;
        for (_permission_key, opportunities) in all_opportunities.iter() {
            // check each of the opportunities for this permission key for validity
            for opportunity in opportunities.iter() {
                let reason = match self
                    .verify_opportunity(VerifyOpportunityInput {
                        opportunity: opportunity.clone().into(),
                    })
                    .await
                {
                    Ok(entities::OpportunityVerificationResult::UnableToSpoof) => {
                        let current_time = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .expect("Current time older than 1970!")
                            .as_micros()
                            as UnixTimestampMicros;
                        if current_time - opportunity.creation_time > MAX_STALE_OPPORTUNITY_MICROS {
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
                        "Removing Opportunity {} for reason {:?}",
                        opportunity.id,
                        reason
                    );
                    if let Err(e) = self
                        .repo
                        .remove_opportunity(&self.db, opportunity, reason.into())
                        .await
                    {
                        tracing::error!("Failed to remove opportunity: {}", e);
                    }
                }
            }
        }
    }
}
