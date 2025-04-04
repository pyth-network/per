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
    #[tracing::instrument(
        skip_all,
        fields(bid_id, profile_name, permission_key, opportunity_id),
        err
    )]
    pub async fn handle_bid(
        &self,
        input: HandleBidInput<T>,
    ) -> Result<entities::Bid<T>, RestError> {
        if let Some(profile) = &input.bid_create.profile {
            tracing::Span::current().record("profile_name", &profile.name);
        }
        let (chain_data, amount) = self
            .verify_bid(VerifyBidInput {
                bid_create: input.bid_create.clone(),
            })
            .await?;
        let bid = self
            .repo
            .add_bid(input.bid_create, &chain_data, &amount)
            .await?;
        tracing::Span::current().record("bid_id", bid.id.to_string());
        Ok(bid)
    }
}
