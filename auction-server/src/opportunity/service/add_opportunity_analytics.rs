use {
    super::{
        get_token_mint::GetTokenMintInput,
        Service,
    },
    crate::{
        api::RestError,
        opportunity::entities,
    },
    std::collections::{
        hash_map::Entry,
        HashMap,
    },
    time::OffsetDateTime,
};

pub struct AddOpportunityAnalyticsInput {
    pub opportunity:    entities::OpportunitySvm,
    pub removal_time:   Option<OffsetDateTime>,
    pub removal_reason: Option<entities::OpportunityRemovalReason>,
}

impl Service {
    pub async fn add_opportunity_analytics(
        &self,
        input: AddOpportunityAnalyticsInput,
    ) -> Result<(), RestError> {
        let mut decimals = HashMap::new();
        for token in input
            .opportunity
            .sell_tokens
            .iter()
            .chain(input.opportunity.buy_tokens.iter())
        {
            if let Entry::Vacant(e) = decimals.entry(token.token) {
                let mint = self
                    .get_token_mint(GetTokenMintInput {
                        chain_id: input.opportunity.chain_id.clone(),
                        mint:     token.token,
                    })
                    .await?;
                e.insert(mint.decimals);
            }
        }
        let prices = self.store.prices.read().await.clone();
        if let Err(err) = self
            .repo
            .add_opportunity_analytics(
                input.opportunity.clone(),
                input.removal_time,
                input.removal_reason,
                prices,
                decimals,
            )
            .await
        {
            tracing::error!(
                error = ?err,
                opportunity = ?input.opportunity,
                "Failed to add opportunity analytics",
            );
            return Err(RestError::TemporarilyUnavailable);
        }
        Ok(())
    }
}
