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
            repository::{
                self,
            },
        },
    },
    ethers::types::Bytes,
    std::{
        fmt::Debug,
        ops::Deref,
    },
    time::OffsetDateTime,
    uuid::Uuid,
};

pub type OpportunityId = Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OpportunityKey(pub ChainId, pub PermissionKey);

#[derive(Debug, Clone, PartialEq)]
pub struct OpportunityCoreFields<T: TokenAmount> {
    pub id:             OpportunityId,
    pub permission_key: Bytes,
    pub chain_id:       ChainId,
    pub sell_tokens:    Vec<T>,
    pub buy_tokens:     Vec<T>,
    pub creation_time:  OffsetDateTime,
}

impl<T: TokenAmount> OpportunityCoreFields<T> {
    pub fn new_with_current_time(val: OpportunityCoreFieldsCreate<T>) -> Self {
        Self {
            id:             Uuid::new_v4(),
            permission_key: val.permission_key,
            chain_id:       val.chain_id,
            sell_tokens:    val.sell_tokens,
            buy_tokens:     val.buy_tokens,
            creation_time:  OffsetDateTime::now_utc(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct OpportunityCoreFieldsCreate<T: TokenAmount> {
    pub permission_key: Bytes,
    pub chain_id:       ChainId,
    pub sell_tokens:    Vec<T>,
    pub buy_tokens:     Vec<T>,
}

// TODO Think more about structure. Isn't it better to have a generic Opportunity struct with a field of type OpportunityParams?
pub trait Opportunity:
    Debug
    + Clone
    + Deref<Target = OpportunityCoreFields<<Self as Opportunity>::TokenAmount>>
    + PartialEq
    + Into<api::Opportunity>
    + Into<Self::OpportunityCreate>
    + TryFrom<repository::Opportunity<Self::ModelMetadata>>
    + Send
    + Sync
{
    type TokenAmount: TokenAmount;
    type ModelMetadata: repository::OpportunityMetadata;
    type OpportunityCreate: OpportunityCreate;

    fn new_with_current_time(val: Self::OpportunityCreate) -> Self;
    fn get_models_metadata(&self) -> Self::ModelMetadata;
    fn get_opportunity_delete(&self) -> api::OpportunityDelete;
    fn get_key(&self) -> OpportunityKey {
        OpportunityKey(self.chain_id.clone(), self.permission_key.clone())
    }
}

pub trait OpportunityCreate: Debug + Clone + From<Self::ApiOpportunityCreate> + PartialEq {
    type ApiOpportunityCreate;

    fn get_key(&self) -> OpportunityKey;
}

#[derive(Debug)]
pub enum OpportunityRemovalReason {
    Expired,
    // TODO use internal errors instead of RestError
    #[allow(dead_code)]
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
