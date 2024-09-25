use {
    super::{
        opportunity::{
            Opportunity,
            OpportunityCoreFields,
        },
        token_amount_svm::TokenAmountSvm,
    },
    solana_sdk::transaction::VersionedTransaction,
    std::ops::Deref,
};

#[derive(Debug, Clone, PartialEq)]
pub struct OpportunitySvm {
    pub core_fields: OpportunityCoreFields<TokenAmountSvm>,

    pub transaction: VersionedTransaction,
}

impl Opportunity for OpportunitySvm {
    type TokenAmount = TokenAmountSvm;
}

impl Deref for OpportunitySvm {
    type Target = OpportunityCoreFields<TokenAmountSvm>;

    fn deref(&self) -> &Self::Target {
        &self.core_fields
    }
}
