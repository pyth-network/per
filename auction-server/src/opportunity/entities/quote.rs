use {
    super::token_amount_svm::TokenAmountSvm,
    crate::{
        kernel::entities::ChainId,
        opportunity::api,
    },
    solana_sdk::{
        pubkey::Pubkey,
        transaction::VersionedTransaction,
    },
};

#[derive(Debug, Clone, PartialEq)]
pub struct Quote {
    pub transaction:                 VersionedTransaction,
    // The expiration time of the quote (in seconds since the Unix epoch)
    pub expiration_time:             i64,
    pub input_token:                 TokenAmountSvm,
    pub output_token:                TokenAmountSvm,
    pub maximum_slippage_percentage: f64,
    pub chain_id:                    ChainId,
}

#[derive(Debug, Clone, PartialEq)]
pub struct QuoteCreate {
    pub user_wallet_address:         Pubkey,
    pub input_token:                 TokenAmountSvm,
    pub output_mint_token:           Pubkey,
    pub maximum_slippage_percentage: f64,
    pub chain_id:                    ChainId,
}

impl From<api::QuoteCreate> for QuoteCreate {
    fn from(quote_create: api::QuoteCreate) -> Self {
        let api::QuoteCreate::Svm(api::QuoteCreateSvm::V1(api::QuoteCreateV1Svm::Phantom(params))) =
            quote_create;

        Self {
            user_wallet_address:         params.user_wallet_address,
            input_token:                 TokenAmountSvm {
                token:  params.input_token_mint,
                amount: params.input_token_amount,
            },
            output_mint_token:           params.output_token_mint,
            maximum_slippage_percentage: params.maximum_slippage_percentage,
            chain_id:                    params.chain_id,
        }
    }
}

impl From<Quote> for api::Quote {
    fn from(quote: Quote) -> Self {
        api::Quote::Svm(api::QuoteSvm::V1(api::QuoteV1Svm {
            transaction:                 quote.transaction,
            expiration_time:             quote.expiration_time,
            input_token:                 quote.input_token.into(),
            output_token:                quote.output_token.into(),
            maximum_slippage_percentage: quote.maximum_slippage_percentage,
            chain_id:                    quote.chain_id,
        }))
    }
}
