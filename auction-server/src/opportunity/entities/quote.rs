use {
    super::token_amount_svm::TokenAmountSvm,
    crate::kernel::entities::ChainId,
    express_relay_api_types::opportunity as api,
    serde::{
        Deserialize,
        Serialize,
    },
    serde_with::{
        serde_as,
        DisplayFromStr,
    },
    solana_sdk::{
        pubkey::Pubkey,
        transaction::VersionedTransaction,
    },
};

#[derive(Debug, Clone, PartialEq)]
pub struct Quote {
    pub transaction:     VersionedTransaction,
    // The expiration time of the quote (in seconds since the Unix epoch)
    pub expiration_time: i64,
    pub input_token:     TokenAmountSvm,
    pub output_token:    TokenAmountSvm,
    pub chain_id:        ChainId,
}

#[derive(Debug, Clone, PartialEq)]
pub struct QuoteCreate {
    pub user_wallet_address: Pubkey,
    pub referral_fee_bps:    u16,
    pub tokens:              QuoteTokens,
    pub router:              Pubkey,
    pub chain_id:            ChainId,
}


#[serde_as]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum QuoteTokens {
    InputTokenSpecified {
        input_token:  TokenAmountSvm,
        #[serde_as(as = "DisplayFromStr")]
        output_token: Pubkey,
    },
    OutputTokenSpecified {
        #[serde_as(as = "DisplayFromStr")]
        input_token:  Pubkey,
        output_token: TokenAmountSvm,
    },
}

impl From<api::QuoteTokens> for QuoteTokens {
    fn from(quote_tokens: api::QuoteTokens) -> Self {
        match quote_tokens {
            api::QuoteTokens::InputTokenSpecified {
                input_token,
                output_token,
                ..
            } => QuoteTokens::InputTokenSpecified {
                input_token: input_token.into(),
                output_token,
            },
            api::QuoteTokens::OutputTokenSpecified {
                input_token,
                output_token,
                ..
            } => QuoteTokens::OutputTokenSpecified {
                input_token,
                output_token: output_token.into(),
            },
        }
    }
}

impl From<api::QuoteCreate> for QuoteCreate {
    fn from(quote_create: api::QuoteCreate) -> Self {
        let api::QuoteCreate::Svm(api::QuoteCreateSvm::V1(params)) = quote_create;

        let tokens = match params.specified_token_amount {
            api::SpecifiedTokenAmount::InputToken { amount } => QuoteTokens::InputTokenSpecified {
                input_token:  TokenAmountSvm {
                    token: params.input_token_mint,
                    amount,
                },
                output_token: params.output_token_mint,
            },
            api::SpecifiedTokenAmount::OutputToken { amount } => {
                QuoteTokens::OutputTokenSpecified {
                    input_token:  params.input_token_mint,
                    output_token: TokenAmountSvm {
                        token: params.output_token_mint,
                        amount,
                    },
                }
            }
        };

        Self {
            user_wallet_address: params.user_wallet_address,
            referral_fee_bps: params.referral_fee_bps,
            tokens,
            router: params.router,
            chain_id: params.chain_id,
        }
    }
}

impl From<Quote> for api::Quote {
    fn from(quote: Quote) -> Self {
        api::Quote::Svm(api::QuoteSvm::V1(api::QuoteV1Svm {
            transaction:     quote.transaction,
            expiration_time: quote.expiration_time,
            input_token:     quote.input_token.into(),
            output_token:    quote.output_token.into(),
            chain_id:        quote.chain_id,
        }))
    }
}
