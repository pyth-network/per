use {
    super::service::{
        ChainTypeEvm,
        Service,
    },
    crate::{
        api::RestError,
        opportunity::{
            repository::models,
            service::verify_opportunity::{
                VerificationResult,
                VerifyOpportunityInput,
            },
        },
        server::{
            EXIT_CHECK_INTERVAL,
            SHOULD_EXIT,
        },
        state::UnixTimestampMicros,
    },
    std::{
        sync::{
            atomic::Ordering,
            Arc,
        },
        time::{
            Duration,
            SystemTime,
            UNIX_EPOCH,
        },
    },
};

const MAX_STALE_OPPORTUNITY_MICROS: i128 = 60_000_000;

#[derive(Debug)]
enum OpportunityRemovalReason {
    Expired,
    Invalid(RestError),
}

impl From<OpportunityRemovalReason> for models::OpportunityRemovalReason {
    fn from(reason: OpportunityRemovalReason) -> Self {
        match reason {
            OpportunityRemovalReason::Expired => models::OpportunityRemovalReason::Expired,
            OpportunityRemovalReason::Invalid(_) => models::OpportunityRemovalReason::Invalid,
        }
    }
}

pub async fn run_verification_loop(service: Arc<Service<ChainTypeEvm>>) -> anyhow::Result<()> {
    tracing::info!("Starting opportunity verifier...");
    let mut exit_check_interval = tokio::time::interval(EXIT_CHECK_INTERVAL);

    // this should be replaced by a subscription to the chain and trigger on new blocks
    let mut submission_interval = tokio::time::interval(Duration::from_secs(5));
    while !SHOULD_EXIT.load(Ordering::Acquire) {
        tokio::select! {
            _ = submission_interval.tick() => {
                let all_opportunities = service.repo.get_opportunities().await;
                for (_permission_key,opportunities) in all_opportunities.iter() {
                    // check each of the opportunities for this permission key for validity
                    for opportunity in opportunities.iter() {
                        let reason = match service.verify_opportunity(VerifyOpportunityInput {
                            opportunity: opportunity.clone(),
                        }).await {
                            Ok(VerificationResult::UnableToSpoof) => {
                                let current_time = SystemTime::now()
                                    .duration_since(UNIX_EPOCH)
                                    .expect("Current time older than 1970!")
                                    .as_micros() as UnixTimestampMicros;
                                if current_time - opportunity.creation_time > MAX_STALE_OPPORTUNITY_MICROS {
                                    Some(OpportunityRemovalReason::Expired)
                                } else {
                                    None
                                }
                            }
                            Err(e) => {
                                if let RestError::InvalidOpportunity(_) = e {
                                    Some(OpportunityRemovalReason::Invalid(e))
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
                            if let Err(e) = service.repo.remove_opportunity(&service.db, opportunity, reason.into()).await {
                                tracing::error!("Failed to remove opportunity: {}", e);
                            }
                        }
                    }
                }
            }
            _ = exit_check_interval.tick() => {
            }
        }
    }
    tracing::info!("Shutting down opportunity verifier...");
    Ok(())
}
