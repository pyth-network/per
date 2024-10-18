use {
    super::{
        ChainTypeSvm,
        Service,
    },
    crate::{
        api::RestError,
        opportunity::entities,
    },
};

pub struct EstimatePriceInput {
    pub quote_create: entities::QuoteCreate,
}

impl Service<ChainTypeSvm> {
    #[tracing::instrument(skip_all)]
    pub async fn estimate_price(&self, _input: EstimatePriceInput) -> Result<u64, RestError> {
        // TODO implement
        return Ok(0);
    }
}
