use {
    super::Repository,
    crate::{
        api::RestError,
        kernel::entities::{
            ChainId,
            PermissionKey,
        },
        opportunity::entities::OpportunitySvm,
    },
    time::OffsetDateTime,
};

impl Repository {
    pub async fn get_opportunities(
        &self,
        chain_id: ChainId,
        permission_key: Option<PermissionKey>,
        from_time: Option<OffsetDateTime>,
    ) -> Result<Vec<OpportunitySvm>, RestError> {
        self.db
            .get_opportunities(chain_id, permission_key, from_time)
            .await
    }
}
