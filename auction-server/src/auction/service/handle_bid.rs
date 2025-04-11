use {
    super::{
        verification::{
            Verification,
            VerifyBidInput,
        },
        Service,
    },
    crate::{
        api::RestError,
        auction::entities,
    },
};

pub struct HandleBidInput {
    pub bid_create: entities::BidCreate,
}

impl Service
{
    #[tracing::instrument(
        skip_all,
        fields(bid_id, profile_name, permission_key, opportunity_id),
        err(level = tracing::Level::TRACE)
    )]
    pub async fn handle_bid(
        &self,
        input: HandleBidInput,
    ) -> Result<entities::Bid, RestError> {
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
