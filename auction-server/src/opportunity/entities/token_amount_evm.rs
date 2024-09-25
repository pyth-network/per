use {
    super::token_amount::TokenAmount,
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
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TokenAmountEvm {
    pub token:  Address,
    pub amount: U256,
}
