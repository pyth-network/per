use {
    super::{
        ChainType,
        Service,
    },
    crate::{
        api::RestError,
        kernel::entities::ChainId,
        opportunity::repository::OpportunityTable,
    },
};

impl<T: ChainType, U: OpportunityTable<T::InMemoryStore>> Service<T, U> {
    pub fn get_config(&self, chain_id: &ChainId) -> Result<&T::Config, RestError> {
        self.config
            .get(chain_id)
            .ok_or(RestError::BadParameters("Chain not found".to_string()))
    }
}
