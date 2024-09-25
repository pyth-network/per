use {
    super::{
        ChainType,
        Service,
    },
    crate::{
        api::RestError,
        kernel::entities::ChainId,
    },
};

impl<T: ChainType> Service<T> {
    pub fn get_config(&self, chain_id: &ChainId) -> Result<&T::Config, RestError> {
        self.config
            .get(chain_id)
            .ok_or(RestError::BadParameters("Config not found".to_string()))
    }
}
