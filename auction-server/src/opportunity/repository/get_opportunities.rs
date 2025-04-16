use {
    super::Repository,
    crate::{
        api::RestError,
        kernel::entities::ChainId,
        opportunity::entities::OpportunitySvm,
    },
    time::OffsetDateTime,
};

impl Repository {
    pub async fn get_opportunities(
        &self,
        chain_id: ChainId,
        from_time: Option<OffsetDateTime>,
    ) -> Result<Vec<OpportunitySvm>, RestError> {
        self.db.get_opportunities(chain_id, from_time).await
    }
}
