use {
    super::token_amount::TokenAmount,
    solana_sdk::pubkey::Pubkey,
};

#[derive(Debug, Clone)]
pub struct TokenAmountSvm {
    pub token:  Pubkey,
    pub amount: u64,
}

impl TokenAmount for TokenAmountSvm {
}
