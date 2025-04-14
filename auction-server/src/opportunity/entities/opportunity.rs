use {
    crate::{
        api::RestError,
        kernel::entities::{
            ChainId,
            PermissionKey,
        },
        opportunity::{
            entities::TokenAmountSvm,
            repository,
        },
    },
    ethers::types::Bytes,
    std::fmt::Debug,
    time::OffsetDateTime,
    uuid::Uuid,
};

pub type OpportunityId = Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OpportunityKey(pub ChainId, pub PermissionKey);

#[derive(Debug, Clone, PartialEq)]
pub struct OpportunityCoreFields {
    pub id:             OpportunityId,
    pub permission_key: Bytes,
    pub chain_id:       ChainId,
    pub sell_tokens:    Vec<TokenAmountSvm>,
    pub buy_tokens:     Vec<TokenAmountSvm>,
    pub creation_time:  OffsetDateTime,
    pub refresh_time:   OffsetDateTime,
}

impl OpportunityCoreFields {
    pub fn new_with_current_time(val: OpportunityCoreFieldsCreate) -> Self {
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
pub struct OpportunityCoreFieldsCreate {
    pub permission_key: Bytes,
    pub chain_id:       ChainId,
    pub sell_tokens:    Vec<TokenAmountSvm>,
    pub buy_tokens:     Vec<TokenAmountSvm>,
}

#[derive(Debug, Clone)]
pub enum OpportunityComparison {
    New,
    Duplicate,
    NeedsRefresh,
}

#[derive(Debug)]
pub enum OpportunityRemovalReason {
    Expired,
    // TODO use internal errors instead of RestError
    #[allow(dead_code)]
    Invalid(RestError),
}

pub enum OpportunityVerificationResult {
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
