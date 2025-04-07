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
    tracing::Level,
};

pub struct GetBidsInput {
    pub profile:   Profile,
    pub from_time: Option<OffsetDateTime>,
}

impl<T: ChainTrait> Service<T> {
    #[tracing::instrument(skip_all, err(level = Level::INFO))]
    pub async fn get_bids(&self, input: GetBidsInput) -> Result<Vec<entities::Bid<T>>, RestError> {
        self.repo.get_bids(input.profile.id, input.from_time).await
    }
}
