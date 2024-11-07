use {
    super::{
        Service,
        ServiceTrait,
    },
    crate::{
        api::RestError,
        bid::entities,
        models::Profile,
    },
    time::OffsetDateTime,
};

pub struct GetBidsInput {
    pub profile:   Profile,
    pub from_time: Option<OffsetDateTime>,
}

impl<T: ServiceTrait> Service<T> {
    pub async fn get_bids(&self, input: GetBidsInput) -> Result<Vec<entities::Bid<T>>, RestError> {
        self.repo
            .get_bids(
                self.config.chain_id.clone(),
                input.profile.id,
                input.from_time,
            )
            .await
    }
}
