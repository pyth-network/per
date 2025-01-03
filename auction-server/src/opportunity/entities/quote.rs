use {
    super::token_amount_svm::TokenAmountSvm,
    crate::kernel::entities::ChainId,
    express_relay_api_types::opportunity as api,
    solana_sdk::{
        pubkey::Pubkey,
        transaction::VersionedTransaction,
    },
};

#[derive(Debug, Clone, PartialEq)]
pub struct Quote {
    pub transaction:          VersionedTransaction,
    // The expiration time of the quote (in seconds since the Unix epoch)
    pub expiration_time:      i64,
    pub input_token:          TokenAmountSvm,
    pub output_token:         TokenAmountSvm,
    pub maximum_slippage_bps: u16,
    pub chain_id:             ChainId,
}

#[derive(Debug, Clone, PartialEq)]
pub struct QuoteCreate {
    pub user_wallet_address:  Pubkey,
    pub tokens:               QuoteTokens,
    pub maximum_slippage_bps: u16,
    pub router:               Pubkey,
    pub chain_id:             ChainId,
}

#[derive(Debug, Clone, PartialEq)]
pub enum QuoteTokens {
    InputTokenSpecified {
        input_token:  TokenAmountSvm,
        output_token: Pubkey,
    },
    OutputTokenSpecified {
        input_token:  Pubkey,
        output_token: TokenAmountSvm,
    },
}

impl From<api::QuoteCreate> for QuoteCreate {
    fn from(quote_create: api::QuoteCreate) -> Self {
        let api::QuoteCreate::Svm(api::QuoteCreateSvm::V1(params)) = quote_create;

        let tokens = match params.specified_token_amount {
            api::SpecifiedTokenAmount::InputToken { amount } => QuoteTokens::InputTokenSpecified {
                input_token:  TokenAmountSvm {
                    token:  params.input_token_mint,
                    amount: amount,
                },
                output_token: params.output_token_mint,
            },
            api::SpecifiedTokenAmount::OutputToken { amount } => {
                QuoteTokens::OutputTokenSpecified {
                    input_token:  params.input_token_mint,
                    output_token: TokenAmountSvm {
                        token:  params.output_token_mint,
                        amount: amount,
                    },
                }
            }
        };

        Self {
            user_wallet_address: params.user_wallet_address,
            tokens,
            maximum_slippage_bps: params.maximum_slippage_bps,
            router: params.router,
            chain_id: params.chain_id,
        }
    }
}

impl From<Quote> for api::Quote {
    fn from(quote: Quote) -> Self {
        api::Quote::Svm(api::QuoteSvm::V1(api::QuoteV1Svm {
            transaction:          quote.transaction,
            expiration_time:      quote.expiration_time,
            input_token:          quote.input_token.into(),
            output_token:         quote.output_token.into(),
            maximum_slippage_bps: quote.maximum_slippage_bps,
            chain_id:             quote.chain_id,
        }))
    }
}
