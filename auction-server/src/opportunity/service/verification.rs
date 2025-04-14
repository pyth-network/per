use {
    super::Service,
    crate::{
        api::RestError,
        opportunity::{
            entities,
            entities::OpportunityCreateSvm,
        },
    },
};

pub struct VerifyOpportunityInput {
    pub opportunity: OpportunityCreateSvm,
}

impl Service {
    pub async fn verify_opportunity(
        &self,
        input: VerifyOpportunityInput,
    ) -> Result<entities::OpportunityVerificationResult, RestError> {
        self.get_config(&input.opportunity.core_fields.chain_id)?;

        // To make sure it'll be expired after a minute
        // TODO - change this to a more realistic value
        Ok(entities::OpportunityVerificationResult::UnableToSpoof)
    }
}
