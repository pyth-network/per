use {
    express_relay_api_types::opportunity as api,
    serde::{
        Deserialize,
        Serialize,
    },
    serde_with::{
        serde_as,
        DisplayFromStr,
    },
    solana_sdk::pubkey::Pubkey,
};

#[serde_as]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TokenAmountSvm {
    #[serde_as(as = "DisplayFromStr")]
    pub token:  Pubkey,
    pub amount: u64,
}

impl From<TokenAmountSvm> for api::TokenAmountSvm {
    fn from(val: TokenAmountSvm) -> Self {
        api::TokenAmountSvm {
            token:  val.token,
            amount: val.amount,
        }
    }
}

impl From<api::TokenAmountSvm> for TokenAmountSvm {
    fn from(val: api::TokenAmountSvm) -> Self {
        Self {
            token:  val.token,
            amount: val.amount,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TokenMint {
    pub mint:     Pubkey,
    pub decimals: u8,
    pub owner:    Pubkey,
}
