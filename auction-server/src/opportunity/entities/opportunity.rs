use {
    super::token_amount::TokenAmount,
    crate::{
        kernel::entities::ChainId,
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
    Clone + Deref<Target = OpportunityCoreFields<<Self as Opportunity>::TokenAmount>> + PartialEq
{
    type TokenAmount: TokenAmount;
}
