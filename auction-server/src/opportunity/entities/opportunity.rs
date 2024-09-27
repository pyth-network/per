use {
    super::token_amount::TokenAmount,
    crate::{
        api::RestError,
        kernel::entities::ChainId,
        opportunity::{
            api,
            repository,
        },
        state::UnixTimestampMicros,
    },
    ethers::types::Bytes,
    std::ops::Deref,
    uuid::Uuid,
};

pub type OpportunityId = Uuid;

#[derive(Debug, Clone, PartialEq)]
pub struct OpportunityCoreFields<T: TokenAmount> {
    pub id:             OpportunityId,
    pub permission_key: Bytes,
    pub chain_id:       ChainId,
    pub sell_tokens:    Vec<T>,
    pub buy_tokens:     Vec<T>,
    pub creation_time:  UnixTimestampMicros,
}

// TODO Need a new entity for CreateOpportunity
pub trait Opportunity:
    std::fmt::Debug
    + Clone
    + Deref<Target = OpportunityCoreFields<<Self as Opportunity>::TokenAmount>>
    + PartialEq
    + Into<Self::ModelMetadata>
    + Into<api::Opportunity>
    + From<Self::ApiOpportunityCreate>
{
    type TokenAmount: TokenAmount;
    type ModelMetadata: repository::OpportunityMetadata;
    type ApiOpportunityCreate;
}

#[derive(Debug)]
pub enum OpportunityRemovalReason {
    Expired,
    // TODO use internal errors instead of RestError
    Invalid(RestError),
}

pub enum OpportunityVerificationResult {
    Success,
    UnableToSpoof,
}

impl From<OpportunityRemovalReason> for repository::OpportunityRemovalReason {
    fn from(reason: OpportunityRemovalReason) -> Self {
        match reason {
            OpportunityRemovalReason::Expired => repository::OpportunityRemovalReason::Expired,
            OpportunityRemovalReason::Invalid(_) => repository::OpportunityRemovalReason::Invalid,
        }
    }
}
