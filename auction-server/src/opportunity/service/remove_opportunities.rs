use {
    super::Service,
    crate::{
        api::{
            ws::UpdateEvent,
            RestError,
        },
        auction::entities::BidPaymentInstructionType,
        kernel::entities::ChainId,
        opportunity::{
            entities::{
                self,
            },
            repository::{
                self,
            },
        },
    },
    solana_sdk::pubkey::Pubkey,
};

pub struct RemoveOpportunitiesInput {
    pub chain_id:           ChainId,
    pub permission_account: Pubkey,
    pub router:             Pubkey,
}

impl Service {
    pub async fn remove_opportunities(
        &self,
        input: RemoveOpportunitiesInput,
    ) -> Result<(), RestError> {
        self.get_config(&input.chain_id)?;
        let permission_key = entities::OpportunitySvm::get_permission_key(
            BidPaymentInstructionType::SubmitBid,
            input.router,
            input.permission_account,
        );
        let opportunities = self
            .repo
            .remove_opportunities(
                &entities::OpportunityKey(input.chain_id.clone(), permission_key),
                repository::OpportunityRemovalReason::Invalid,
            )
            .await
            .map_err(|e| {
                tracing::error!(
                    error = ?e,
                    chain_id = input.chain_id,
                    permission_key =
                    "Failed to remove opportunities",
                );
                RestError::TemporarilyUnavailable
            })?;

        if !opportunities.is_empty() {
            let opportunity = opportunities[0].clone();
            self.store
                .ws
                .broadcast_sender
                .send(UpdateEvent::RemoveOpportunities(
                    opportunity.get_opportunity_delete(),
                ))
                .map_err(|e| {
                    tracing::error!(
                        error = e.to_string(),
                        opportunities = ?opportunities,
                        "Failed to send remove opportunities",
                    );
                    RestError::TemporarilyUnavailable
                })?;
        }

        Ok(())
    }
}
