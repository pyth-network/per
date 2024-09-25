use {
    super::token_amount::TokenAmount,
    serde::{
        Deserialize,
        Serialize,
    },
    solana_sdk::pubkey::Pubkey,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TokenAmountSvm {
    pub token:  Pubkey,
    pub amount: u64,
}

impl TokenAmount for TokenAmountSvm {
}
