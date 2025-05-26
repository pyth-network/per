use {
    super::token::TokenAmountSvm,
    crate::{
        kernel::entities::ChainId,
        models,
    },
    express_relay_api_types::{
        bid::BidId,
        opportunity as api,
    },
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
    pub transaction:     Option<VersionedTransaction>,
    // The expiration time of the quote (in seconds since the Unix epoch)
    pub expiration_time: Option<i64>,
    pub searcher_token:  TokenAmountSvm,
    pub user_token:      TokenAmountSvm,
    pub referrer_fee:    TokenAmountSvm,
    pub platform_fee:    TokenAmountSvm,
    pub chain_id:        ChainId,
    pub reference_id:    BidId,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReferralFeeInfo {
    pub router:           Pubkey,
    pub referral_fee_ppm: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct QuoteCreate {
    pub user_wallet_address: Option<Pubkey>,
    pub tokens:              QuoteTokens,
    pub referral_fee_info:   Option<ReferralFeeInfo>,
    pub chain_id:            ChainId,
    pub memo:                Option<String>,
    pub cancellable:         bool,
    pub minimum_lifetime:    Option<u32>,
    pub profile_id:          Option<models::ProfileId>,
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
            referral_fee_ppm: referral_fee_info.referral_fee_ppm,
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
            reference_id:    quote.reference_id,
        }))
    }
}
