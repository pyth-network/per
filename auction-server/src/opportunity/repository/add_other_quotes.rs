use {
    super::Repository,
    crate::{
        api::RestError,
        opportunity::entities::{
            OtherQuote,
            QuoteCreate,
            QuoteTokens,
        },
    },
    governor::RateLimiter,
    jupiter_swap_api_client::{
        quote::{
            ParamsMode,
            SwapMode as UltraSwapMode,
            SwapType,
            UltraQuoteRequest,
        },
        JupiterUltraSwapApiClient,
    },
    std::sync::Arc,
    uuid::Uuid,
};

pub const QUOTER_ULTRA: &str = "JupiterUltra";
pub const RATE_LIMIT_ULTRA_PER_SECOND: u32 = 1;

impl Repository {
    pub async fn add_other_quotes(
        &self,
        opportunity_id: Uuid,
        jupiter_ultra_client: JupiterUltraSwapApiClient,
        quote_create: QuoteCreate,
    ) -> Result<(), RestError> {
        let other_quotes = self
            .get_other_quotes(jupiter_ultra_client, quote_create)
            .await;
        self.db.add_other_quotes(opportunity_id, other_quotes).await
    }

    async fn get_other_quotes(
        &self,
        jupiter_ultra_client: JupiterUltraSwapApiClient,
        quote_create: QuoteCreate,
    ) -> Vec<OtherQuote> {
        let (input_mint, output_mint, amount, swap_mode) = match quote_create.tokens {
            QuoteTokens::UserTokenSpecified {
                user_token,
                searcher_token,
            } => (
                user_token.token,
                searcher_token,
                user_token.amount,
                Some(UltraSwapMode::ExactIn),
            ),
            QuoteTokens::SearcherTokenSpecified {
                user_token,
                searcher_token,
            } => (
                user_token,
                searcher_token.token,
                searcher_token.amount,
                Some(UltraSwapMode::ExactOut),
            ),
        };
        let mut other_quotes = vec![];

        let limiter = self
            .last_other_quotes_call
            .entry(QUOTER_ULTRA.to_string())
            .or_insert_with(|| {
                Arc::new(RateLimiter::direct(governor::Quota::per_second(
                    std::num::NonZeroU32::new(RATE_LIMIT_ULTRA_PER_SECOND).unwrap(),
                )))
            })
            .clone();
        if limiter.check().is_ok() {
            let ultra_response = jupiter_ultra_client
                .quote(&UltraQuoteRequest {
                    input_mint,
                    output_mint,
                    amount,
                    swap_mode: swap_mode.clone(),
                    mode: ParamsMode::Ultra,
                    taker: quote_create.user_wallet_address,
                })
                .await;
            if let Ok(ultra_response) = ultra_response {
                other_quotes.push(OtherQuote {
                    quoter:        "JupiterUltra".to_string(),
                    amount_quoted: match swap_mode {
                        None | Some(UltraSwapMode::ExactIn) => ultra_response.out_amount,
                        Some(UltraSwapMode::ExactOut) => ultra_response.in_amount,
                    },
                    slippage_bps:  Some(ultra_response.slippage_bps),
                    fee_mint:      ultra_response.fee_mint,
                    fee_bps:       Some(ultra_response.fee_bps),
                    deadline:      ultra_response.expire_at,
                    transaction:   ultra_response.transaction.clone(),
                    swap_details:  match ultra_response.swap_type {
                        SwapType::Aggregator => "aggregator".to_string(),
                        SwapType::Rfq => "rfq".to_string(),
                    },
                });
            }
        }

        other_quotes
    }
}
