use {
    super::{
        ChainType,
        Service,
    },
    crate::api::RestError,
};

impl<T: ChainType> Service<T> {
    pub async fn clear_opportunities_upon_restart(&self) -> Result<(), RestError> {
        self.repo.clear_opportunities_upon_restart().await
    }
}
