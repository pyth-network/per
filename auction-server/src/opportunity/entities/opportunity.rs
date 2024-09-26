use {
    super::token_amount::TokenAmount,
    crate::{
        api::RestError,
        kernel::entities::ChainId,
        opportunity::{
            api::OpportunityParamsWithMetadata,
            repository::models::OpportunityMetadata,
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

pub trait Opportunity:
    std::fmt::Debug
    + Clone
    + Deref<Target = OpportunityCoreFields<<Self as Opportunity>::TokenAmount>>
    + PartialEq
    + Into<Self::Metadata>
    + Into<OpportunityParamsWithMetadata>
{
    type TokenAmount: TokenAmount;
    type Metadata: OpportunityMetadata;
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
