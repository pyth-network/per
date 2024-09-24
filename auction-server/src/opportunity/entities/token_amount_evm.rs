use {
    super::token_amount::TokenAmount,
    ethers::types::{
        Address,
        U256,
    },
};

impl TokenAmount for TokenAmountEvm {
}

#[derive(Debug, Clone)]
pub struct TokenAmountEvm {
    pub token:  Address,
    pub amount: U256,
}
