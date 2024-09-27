use {
    super::token_amount::TokenAmount,
    crate::opportunity::api,
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

impl TokenAmount for TokenAmountSvm {
    type ApiTokenAmount = api::TokenAmountSvm;
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
