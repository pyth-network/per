use {
    super::{
        opportunity::{
            Opportunity,
            OpportunityCoreFields,
        },
        token_amount_svm::TokenAmountSvm,
    },
    crate::opportunity::repository::models::OpportunityMetadataSvm,
    solana_sdk::pubkey::Pubkey,
    std::ops::Deref,
};

#[derive(Debug, Clone, PartialEq)]
pub struct OpportunitySvm {
    pub core_fields: OpportunityCoreFields<TokenAmountSvm>,

    pub router:     Pubkey,
    pub permission: Pubkey,
}

impl Opportunity for OpportunitySvm {
    type TokenAmount = TokenAmountSvm;
    type Metadata = OpportunityMetadataSvm;
}

impl Deref for OpportunitySvm {
    type Target = OpportunityCoreFields<TokenAmountSvm>;

    fn deref(&self) -> &Self::Target {
        &self.core_fields
    }
}
