#[cfg(test)]
pub use test::MockOpportunity;
use {
    super::token_amount::TokenAmount,
    crate::{
        api::RestError,
        kernel::entities::{
            ChainId,
            PermissionKey,
        },
        opportunity::repository,
    },
    ethers::types::Bytes,
    express_relay_api_types::opportunity as api,
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
    pub refresh_time:   OffsetDateTime,
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
            refresh_time:   OffsetDateTime::now_utc(),
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

#[derive(Debug, Clone)]
pub enum OpportunityComparison {
    New,
    Duplicate,
    NeedsRefresh,
}

// TODO Think more about structure. Isn't it better to have a generic Opportunity struct with a field of type OpportunityParams?
pub trait Opportunity:
    Debug
    + Clone
    + Deref<Target = OpportunityCoreFields<<Self as Opportunity>::TokenAmount>>
    + PartialEq
    + Into<api::Opportunity>
    + Into<Self::OpportunityCreateAssociatedType>
    + TryFrom<repository::Opportunity<Self::ModelMetadata>>
    + Send
    + Sync
{
    type TokenAmount: TokenAmount;
    type ModelMetadata: repository::OpportunityMetadata;
    type OpportunityCreateAssociatedType: OpportunityCreate;

    fn new_with_current_time(val: Self::OpportunityCreateAssociatedType) -> Self;
    fn get_models_metadata(&self) -> Self::ModelMetadata;
    fn get_opportunity_delete(&self) -> api::OpportunityDelete;
    fn get_key(&self) -> OpportunityKey {
        OpportunityKey(self.chain_id.clone(), self.permission_key.clone())
    }

    fn compare(&self, other: &Self::OpportunityCreateAssociatedType) -> OpportunityComparison;
    fn refresh(&mut self);
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

#[cfg(test)]
pub mod test {
    use {
        super::{
            super::token_amount::test::MockTokenAmount,
            *,
        },
        mockall::mock,
        repository::MockOpportunityMetadata,
        serde::{
            Deserialize,
            Serialize,
        },
    };


    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
    pub struct MockOpportunityCreate {}

    impl OpportunityCreate for MockOpportunityCreate {
        type ApiOpportunityCreate = MockOpportunityCreate;

        fn get_key(&self) -> OpportunityKey {
            OpportunityKey(ChainId::default(), PermissionKey::default())
        }
    }


    mock! {
        pub Opportunity{}

        impl Opportunity for Opportunity {
            type TokenAmount = MockTokenAmount;
            type ModelMetadata = MockOpportunityMetadata;
            type OpportunityCreateAssociatedType = MockOpportunityCreate;

            fn new_with_current_time(val: <MockOpportunity as Opportunity>::OpportunityCreateAssociatedType) -> Self;
            fn get_models_metadata(&self) -> <MockOpportunity as Opportunity>::ModelMetadata;
            fn get_opportunity_delete(&self) -> api::OpportunityDelete;
            fn get_key(&self) -> OpportunityKey;
            fn compare(&self, other: &<MockOpportunity as Opportunity>::OpportunityCreateAssociatedType) -> OpportunityComparison;
            fn refresh(&mut self);
        }

        impl Deref for Opportunity {
            type Target = OpportunityCoreFields<MockTokenAmount>;

            fn deref(&self) -> &<MockOpportunity as Deref>::Target;
        }

        impl PartialEq for Opportunity {
            fn eq(&self, other: &Self) -> bool;
        }

        impl Clone for Opportunity {
            fn clone(&self) -> Self;
        }

        impl Debug for Opportunity {
            fn fmt<'a>(&self, f: &mut std::fmt::Formatter<'a>) -> std::fmt::Result;
        }

        impl Into<api::Opportunity> for Opportunity {
            fn into(self) -> api::Opportunity;
        }

        impl TryFrom<repository::Opportunity<<MockOpportunity as Opportunity>::ModelMetadata>> for Opportunity {
            type Error = ();
            fn try_from(value: repository::Opportunity<<MockOpportunity as Opportunity>::ModelMetadata>) -> Result<Self, <MockOpportunity as TryFrom<repository::Opportunity<<MockOpportunity as Opportunity>::ModelMetadata>>>::Error>;
        }
    }

    impl Into<<MockOpportunity as Opportunity>::OpportunityCreateAssociatedType> for MockOpportunity {
        fn into(self) -> MockOpportunityCreate {
            MockOpportunityCreate::default()
        }
    }
}
