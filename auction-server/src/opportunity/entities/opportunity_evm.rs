use {
    super::{
        opportunity::{
            Opportunity,
            OpportunityCoreFields,
        },
        token_amount_evm::TokenAmountEvm,
    },
    crate::opportunity::repository::models::OpportunityMetadataEvm,
    ethers::types::{
        Bytes,
        U256,
    },
    std::ops::Deref,
};


#[derive(Debug, Clone, PartialEq)]
pub struct OpportunityEvm {
    pub core_fields: OpportunityCoreFields<TokenAmountEvm>,

    pub target_contract:   ethers::abi::Address,
    pub target_calldata:   Bytes,
    pub target_call_value: U256,
}

impl Opportunity for OpportunityEvm {
    type TokenAmount = TokenAmountEvm;
    type Metadata = OpportunityMetadataEvm;
}

impl Deref for OpportunityEvm {
    type Target = OpportunityCoreFields<TokenAmountEvm>;

    fn deref(&self) -> &Self::Target {
        &self.core_fields
    }
}
