use {
    super::{
        token_amount_svm::TokenAmountSvm,
        OpportunityId,
    },
    crate::kernel::entities::ChainId,
    express_relay_api_types::opportunity as api,
    serde::{
        Deserialize,
        Serialize,
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
    pub searcher_token:  TokenAmountSvm,
    pub user_token:      TokenAmountSvm,
    pub referrer_fee:    TokenAmountSvm,
    pub platform_fee:    TokenAmountSvm,
    pub chain_id:        ChainId,
    pub quote_id:        OpportunityId,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReferralFeeInfo {
    pub router:           Pubkey,
    pub referral_fee_bps: u16,
}

#[derive(Debug, Clone, PartialEq)]
pub struct QuoteCreate {
    pub user_wallet_address: Pubkey,
    pub tokens:              QuoteTokens,
    pub referral_fee_info:   Option<ReferralFeeInfo>,
    pub chain_id:            ChainId,
}


#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum QuoteTokens {
    UserTokenSpecified {
        user_token:     TokenAmountSvm,
        searcher_token: Pubkey,
    },
    SearcherTokenSpecified {
        user_token:     Pubkey,
        searcher_token: TokenAmountSvm,
    },
}

impl From<api::QuoteTokens> for QuoteTokens {
    fn from(quote_tokens: api::QuoteTokens) -> Self {
        match quote_tokens {
            api::QuoteTokens::UserTokenSpecified {
                user_token,
                searcher_token,
                user_amount,
                ..
            } => QuoteTokens::UserTokenSpecified {
                user_token: TokenAmountSvm {
                    token:  user_token,
                    amount: user_amount,
                },
                searcher_token,
            },
            api::QuoteTokens::SearcherTokenSpecified {
                user_token,
                searcher_token,
                searcher_amount,
            } => QuoteTokens::SearcherTokenSpecified {
                user_token,
                searcher_token: TokenAmountSvm {
                    token:  searcher_token,
                    amount: searcher_amount,
                },
            },
        }
    }
}

impl From<api::ReferralFeeInfo> for ReferralFeeInfo {
    fn from(referral_fee_info: api::ReferralFeeInfo) -> Self {
        Self {
            router:           referral_fee_info.router,
            referral_fee_bps: referral_fee_info.referral_fee_bps,
        }
    }
}

impl From<api::QuoteCreate> for QuoteCreate {
    fn from(quote_create: api::QuoteCreate) -> Self {
        let api::QuoteCreate::Svm(api::QuoteCreateSvm::V1(params)) = quote_create;

        let tokens = match params.specified_token_amount {
            api::SpecifiedTokenAmount::InputToken { amount } => QuoteTokens::UserTokenSpecified {
                user_token:     TokenAmountSvm {
                    token: params.input_token_mint,
                    amount,
                },
                searcher_token: params.output_token_mint,
            },
            api::SpecifiedTokenAmount::OutputToken { amount } => {
                QuoteTokens::SearcherTokenSpecified {
                    user_token:     params.input_token_mint,
                    searcher_token: TokenAmountSvm {
                        token: params.output_token_mint,
                        amount,
                    },
                }
            }
        };

        let referral_fee_info = params.referral_fee_info.map(Into::into);

        Self {
            user_wallet_address: params.user_wallet_address,
            tokens,
            referral_fee_info,
            chain_id: params.chain_id,
        }
    }
}

impl From<Quote> for api::Quote {
    fn from(quote: Quote) -> Self {
        api::Quote::Svm(api::QuoteSvm::V1(api::QuoteV1Svm {
            transaction:     quote.transaction,
            expiration_time: quote.expiration_time,
            input_token:     quote.user_token.into(),
            output_token:    quote.searcher_token.into(),
            referrer_fee:    quote.referrer_fee.into(),
            platform_fee:    quote.platform_fee.into(),
            chain_id:        quote.chain_id,
            quote_id:        quote.quote_id,
        }))
    }
}
