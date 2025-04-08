use {
    super::{
        ChainTrait,
        Service,
    },
    crate::{
        api::RestError,
        auction::entities,
        models::Profile,
    },
    time::OffsetDateTime,
};

pub struct GetBidsInput {
    pub profile:   Profile,
    pub from_time: Option<OffsetDateTime>,
}

impl<T: ChainTrait> Service<T> {
    #[tracing::instrument(skip_all, err(level = tracing::Level::TRACE))]
    pub async fn get_bids(&self, input: GetBidsInput) -> Result<Vec<entities::Bid<T>>, RestError> {
        self.repo.get_bids(input.profile.id, input.from_time).await
    }
}
