use {
    super::token_amount::TokenAmount,
    crate::opportunity::api,
    ethers::types::{
        Address,
        U256,
    },
    serde::{
        Deserialize,
        Serialize,
    },
};

impl TokenAmount for TokenAmountEvm {
    type ApiTokenAmount = api::TokenAmountEvm;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TokenAmountEvm {
    pub token:  Address,
    #[serde(with = "crate::serde::u256")]
    pub amount: U256,
}

impl From<TokenAmountEvm> for api::TokenAmountEvm {
    fn from(val: TokenAmountEvm) -> Self {
        api::TokenAmountEvm {
            token:  val.token,
            amount: val.amount,
        }
    }
}

impl From<api::TokenAmountEvm> for TokenAmountEvm {
    fn from(val: api::TokenAmountEvm) -> Self {
        Self {
            token:  val.token,
            amount: val.amount,
        }
    }
}
