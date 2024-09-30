use {
    super::token_amount::TokenAmount,
    crate::{
        api::RestError,
        kernel::entities::{
            ChainId,
            PermissionKey,
        },
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

#[derive(Debug, Clone)]
pub struct OpportunityCoreFieldsCreate<T: TokenAmount> {
    pub permission_key: Bytes,
    pub chain_id:       ChainId,
    pub sell_tokens:    Vec<T>,
    pub buy_tokens:     Vec<T>,
}

// TODO Think more about structure. Isn't it better to have a generic Opportunity struct with a field of type OpportunityParams?
pub trait Opportunity:
    std::fmt::Debug
    + Clone
    + Deref<Target = OpportunityCoreFields<<Self as Opportunity>::TokenAmount>>
    + PartialEq
    + Into<Self::ModelMetadata>
    + Into<api::Opportunity>
    + From<Self::OpportunityCreate>
    + Into<Self::OpportunityCreate>
    + PartialEq<Self::OpportunityCreate>
    + TryFrom<repository::Opportunity<Self::ModelMetadata>>
{
    type TokenAmount: TokenAmount;
    type ModelMetadata: repository::OpportunityMetadata;
    type OpportunityCreate: OpportunityCreate;
}

pub trait OpportunityCreate: std::fmt::Debug + Clone + From<Self::ApiOpportunityCreate> {
    type ApiOpportunityCreate;

    fn permission_key(&self) -> PermissionKey;
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
