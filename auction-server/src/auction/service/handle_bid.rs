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
    #[tracing::instrument(skip_all)]
    pub async fn handle_bid(
        &self,
        input: HandleBidInput<T>,
    ) -> Result<entities::Bid<T>, RestError> {
        let (chain_data, amount) = self
            .verify_bid(VerifyBidInput {
                bid_create: input.bid_create.clone(),
            })
            .await?;
        self.repo
            .add_bid(input.bid_create, &chain_data, &amount)
            .await
    }
}
