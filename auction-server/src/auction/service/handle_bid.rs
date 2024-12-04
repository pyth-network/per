use {
    super::{
        verification::{
            Verification,
            VerifyBidInput,
        },
        ChainTrait,
        Service,
    },
    crate::{
        api::RestError,
        auction::entities,
    },
};

pub struct HandleBidInput<T: ChainTrait> {
    pub bid_create: entities::BidCreate<T>,
}

impl<T: ChainTrait> Service<T>
where
    Service<T>: Verification<T>,
{
    #[tracing::instrument(skip_all, fields(bid_id, profile_name, simulation_error))]
    pub async fn handle_bid(
        &self,
        input: HandleBidInput<T>,
    ) -> Result<entities::Bid<T>, RestError> {
        if let Some(profile) = &input.bid_create.profile {
            tracing::Span::current().record("profile_name", &profile.name);
        }
        let verification = self
            .verify_bid(VerifyBidInput {
                bid_create: input.bid_create.clone(),
            })
            .await;
        if let Err(RestError::SimulationError { result: _, reason }) = &verification {
            // Long values are truncated and the errors are at the end of the simulation logs
            let error = reason.split('\n').rev().collect::<Vec<_>>().join("\n");
            tracing::Span::current().record("simulation_error", error);
        }
        let (chain_data, amount) = verification?;
        let bid = self
            .repo
            .add_bid(input.bid_create, &chain_data, &amount)
            .await?;
        tracing::Span::current().record("bid_id", bid.id.to_string());
        Ok(bid)
    }
}
