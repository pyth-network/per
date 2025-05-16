use {
    super::Repository,
    crate::{
        api::RestError,
        opportunity::entities::OpportunitySvm,
    },
};

impl Repository {
    pub async fn add_opportunity_analytics(
        &self,
        opportunity: OpportunitySvm,
    ) -> Result<(), RestError> {
        self.db_analytics
            .add_opportunity(&opportunity, None, None)
            .await
    }
}
