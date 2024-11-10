use {
    super::{
        verification::{
            Verification,
            VerifyBidInput,
        },
        Service,
        ServiceTrait,
    },
    crate::{
        api::RestError,
        bid::entities,
    },
};

pub struct HandleBidInput<T: entities::BidCreateTrait> {
    // store_new: Arc<StoreNew>,
    pub bid_create: entities::BidCreate<T>,
}

impl<T: ServiceTrait> Service<T>
where
    Service<T>: Verification<T>,
{
    #[tracing::instrument(skip_all)]
    pub async fn handle_bid(&self, input: HandleBidInput<T>) -> Result<entities::BidId, RestError> {
        self.verify_bid(VerifyBidInput {
            bid_create: input.bid_create.clone(),
        })?;
        todo!();
    }
}
