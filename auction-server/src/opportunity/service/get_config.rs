use {
    super::Service,
    crate::{
        api::RestError,
        kernel::entities::ChainId,
        opportunity::service::ConfigSvm,
    },
};

impl Service {
    pub fn get_config(&self, chain_id: &ChainId) -> Result<&ConfigSvm, RestError> {
        self.config
            .get(chain_id)
            .ok_or(RestError::BadParameters("Chain not found".to_string()))
    }
}
