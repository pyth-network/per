use {
    super::{
        ChainType,
        ChainTypeSvm,
        Service,
    },
    crate::{
        api::RestError,
        opportunity::{
            entities,
            repository::InMemoryStore,
        },
    },
    std::future::Future,
};

pub struct VerifyOpportunityInput<T: entities::OpportunityCreate> {
    pub opportunity: T,
}

pub trait Verification<T: ChainType> {
    fn verify_opportunity(
        &self,
        input: VerifyOpportunityInput<<<T::InMemoryStore as InMemoryStore>::Opportunity as entities::Opportunity>::OpportunityCreate>,
    ) -> impl Future<Output = Result<entities::OpportunityVerificationResult, RestError>>;
}

impl Verification<ChainTypeSvm> for Service<ChainTypeSvm> {
    async fn verify_opportunity(
        &self,
        input: VerifyOpportunityInput<entities::OpportunityCreateSvm>,
    ) -> Result<entities::OpportunityVerificationResult, RestError> {
        self.get_config(&input.opportunity.core_fields.chain_id)?;

        // To make sure it'll be expired after a minute
        // TODO - change this to a more realistic value
        Ok(entities::OpportunityVerificationResult::UnableToSpoof)
    }
}
