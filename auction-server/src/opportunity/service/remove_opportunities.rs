use {
    super::{
        ChainTypeSvm,
        Service,
    },
    crate::{
        api::{
            ws::UpdateEvent,
            RestError,
        },
        kernel::entities::ChainId,
        opportunity::{
            entities,
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

impl Service<ChainTypeSvm> {
    pub async fn remove_opportunities(
        &self,
        input: RemoveOpportunitiesInput,
    ) -> Result<(), RestError> {
        self.get_config(&input.chain_id)?;
        let opportunities = self
            .repo
            .remove_opportunities(
                &self.db,
                entities::OpportunitySvm::get_permission_key(
                    input.router,
                    input.permission_account,
                ),
                repository::OpportunityRemovalReason::Invalid,
            )
            .await;

        if !opportunities.is_empty() {
            let opportunity = opportunities[0].clone();
            self.store
                .ws
                .broadcast_sender
                .send(UpdateEvent::RemoveOpportunities(opportunity.into()))
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
