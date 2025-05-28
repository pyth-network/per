use {
    super::{
        BidAnalyticsLimo,
        BidAnalyticsSwap,
        Repository,
    },
    crate::{
        auction::entities::{
            self,
            BidStatus,
        },
        kernel::{
            entities::Svm,
            pyth_lazer::calculate_notional_value,
        },
        state::Price,
    },
    base64::{
        engine::general_purpose::STANDARD,
        Engine as _,
    },
    express_relay::{
        SubmitBidArgs,
        SwapV2Args,
    },
    solana_sdk::pubkey::Pubkey,
    std::collections::HashMap,
};

impl Repository {
    #[tracing::instrument(skip_all, err(level = tracing::Level::TRACE))]
    pub async fn add_bid_analytics(
        &self,
        bid: entities::Bid,
        data: entities::BidTransactionData,
        prices: HashMap<Pubkey, Price>,
        decimals: HashMap<Pubkey, u8>,
    ) -> anyhow::Result<()> {
        let transaction = STANDARD.encode(bincode::serialize(&bid.chain_data.transaction.clone())?);
        let bid_analytics = match data {
            entities::BidTransactionData::SubmitBid(transaction_data) => {
                let SubmitBidArgs {
                    deadline,
                    bid_amount: _,
                } = transaction_data.data;
                let bid_analytics = BidAnalyticsLimo {
                    id: bid.id,
                    creation_time: bid.creation_time,
                    initiation_time: bid.initiation_time,
                    permission_key: bid.chain_data.get_permission_key().to_string(),
                    chain_id: bid.chain_id,
                    transaction,
                    bid_amount: bid.amount,

                    auction_id: bid.status.get_auction_id(),
                    submission_time: bid.submission_time,
                    conclusion_time: bid.conclusion_time,

                    status: serde_json::to_string(&Svm::convert_bid_status(&bid.status))?,

                    router: bid.chain_data.router.to_string(),
                    permission_account: bid.chain_data.permission_account.to_string(),
                    deadline,

                    profile_id: bid.profile_id,
                };
                super::BidAnalytics::Limo(bid_analytics)
            }
            entities::BidTransactionData::Swap(transaction_data) => {
                let status_reason = Svm::get_bid_status_reason(&bid.status);
                let mint_user = transaction_data.accounts.mint_user;
                let user_token_notional_usd_value = calculate_notional_value(
                    prices.get(&mint_user).cloned(),
                    transaction_data.data.amount_user,
                    decimals.get(&mint_user).cloned(),
                );
                let mint_searcher = transaction_data.accounts.mint_searcher;
                let searcher_token_notional_usd_value = calculate_notional_value(
                    prices.get(&mint_searcher).cloned(),
                    transaction_data.data.amount_searcher,
                    decimals.get(&mint_searcher).cloned(),
                );

                let SwapV2Args {
                    fee_token,
                    amount_searcher,
                    amount_user,
                    referral_fee_ppm,
                    swap_platform_fee_ppm,
                    deadline,
                } = transaction_data.data;
                let entities::SwapAccounts {
                    searcher,
                    user_wallet,
                    mint_searcher,
                    mint_user,
                    router_token_account,
                    token_program_searcher,
                    token_program_user,
                } = transaction_data.accounts;
                let fee_token = match fee_token {
                    ::express_relay::FeeToken::Searcher => "searcher",
                    ::express_relay::FeeToken::User => "user",
                };
                let bid_analytics = BidAnalyticsSwap {
                    id: bid.id,
                    creation_time: bid.creation_time,
                    initiation_time: bid.initiation_time,
                    submission_time: bid.submission_time,
                    permission_key: bid.chain_data.get_permission_key().to_string(),
                    chain_id: bid.chain_id,
                    transaction,
                    bid_amount: bid.amount,

                    auction_id: bid.status.get_auction_id(),
                    // TODO Fill this in
                    opportunity_id: bid.opportunity_id,
                    conclusion_time: bid.conclusion_time,

                    searcher_token_mint: mint_searcher.to_string(),
                    searcher_token_amount: amount_searcher,
                    searcher_token_notional_usd_value,

                    user_token_mint: mint_user.to_string(),
                    user_token_amount: amount_user,
                    user_token_notional_usd_value,

                    status: serde_json::to_string(&Svm::convert_bid_status(&bid.status))?,
                    status_reason: status_reason
                        .map(|status_reason| serde_json::to_string(&status_reason))
                        .transpose()?,

                    user_wallet_address: user_wallet.to_string(),
                    searcher_wallet_address: searcher.to_string(),
                    fee_token: fee_token.to_string(),
                    referral_fee_ppm,
                    platform_fee_ppm: swap_platform_fee_ppm,
                    deadline,
                    token_program_user: token_program_user.to_string(),
                    token_program_searcher: token_program_searcher.to_string(),
                    router_token_account: router_token_account.to_string(),

                    profile_id: bid.profile_id,
                };
                super::BidAnalytics::Swap(bid_analytics)
            }
        };
        self.db_analytics.add_bid(bid_analytics).await
    }
}
