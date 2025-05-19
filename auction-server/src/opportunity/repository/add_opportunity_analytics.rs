use {
    super::Repository,
    crate::opportunity::entities,
    time::OffsetDateTime,
};

impl Repository {
    pub async fn add_opportunity_analytics(
        &self,
        opportunity: entities::OpportunitySvm,
        removal_time: Option<OffsetDateTime>,
        removal_reason: Option<entities::OpportunityRemovalReason>,
    ) -> anyhow::Result<()> {
        self.db_analytics
            .add_opportunity(&opportunity, removal_time, removal_reason.map(|r| r.into()))
            .await
    }
}
