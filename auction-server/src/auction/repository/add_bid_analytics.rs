use {
    super::{
        BidAnalyticsLimo,
        BidAnalyticsSwap,
        BidStatusReason,
        Repository,
    },
    crate::{
        auction::entities::{
            self,
            BidStatus,
        },
        kernel::entities::Svm,
    },
    base64::{
        engine::general_purpose::STANDARD,
        Engine as _,
    },
};

impl Repository {
    #[tracing::instrument(skip_all, err(level = tracing::Level::TRACE))]
    pub async fn add_bid_analytics(
        &self,
        bid: entities::Bid,
        data: entities::BidTransactionData,
    ) -> anyhow::Result<()> {
        let transaction = STANDARD.encode(bincode::serialize(&bid.chain_data.transaction.clone())?);
        let status_reason: Option<BidStatusReason> = bid.status.clone().into();
        let bid_analytics = match data {
            entities::BidTransactionData::SubmitBid(transaction_data) => {
                let bid_analytics = BidAnalyticsLimo {
                    id: bid.id,
                    creation_time: bid.creation_time,
                    initiation_time: bid.initiation_time,
                    permission_key: format!("{}", bid.chain_data.get_permission_key()),
                    chain_id: bid.chain_id,
                    transaction,
                    bid_amount: bid.amount,

                    auction_id: bid.status.get_auction_id(),
                    conclusion_time: bid.conclusion_time,

                    status: serde_json::to_string(&Svm::convert_bid_status(&bid.status))
                        .expect("Failed to serialize bid status"),

                    router: bid.chain_data.router.to_string(),
                    permission_account: bid.chain_data.permission_account.to_string(),
                    deadline: transaction_data.data.deadline,

                    profile_id: bid.profile_id,
                };
                super::BidAnalytics::Limo(bid_analytics)
            }
            entities::BidTransactionData::Swap(transaction_data) => {
                let fee_token = match transaction_data.data.fee_token {
                    ::express_relay::FeeToken::Searcher => "searcher",
                    ::express_relay::FeeToken::User => "user",
                };
                let bid_analytics = BidAnalyticsSwap {
                    id: bid.id,
                    creation_time: bid.creation_time,
                    initiation_time: bid.initiation_time,
                    permission_key: format!("{}", bid.chain_data.get_permission_key()),
                    chain_id: bid.chain_id,
                    transaction,
                    bid_amount: bid.amount,

                    auction_id: bid.status.get_auction_id(),
                    // TODO Fill this in
                    opportunity_id: None,
                    conclusion_time: bid.conclusion_time,

                    searcher_token_mint: transaction_data.accounts.mint_searcher.to_string(),
                    searcher_token_amount: transaction_data.data.amount_searcher,
                    // TODO Fill this in
                    searcher_token_usd_price: None,

                    user_token_mint: transaction_data.accounts.mint_user.to_string(),
                    user_token_amount: transaction_data.data.amount_user,
                    // TODO Fill this in
                    user_token_usd_price: None,

                    status: serde_json::to_string(&Svm::convert_bid_status(&bid.status))
                        .expect("Failed to serialize bid status"),
                    status_reason: status_reason.map(|status_reason| {
                        serde_json::to_string(&status_reason)
                            .expect("Failed to serialize status reason")
                    }),

                    user_wallet_address: transaction_data.accounts.user_wallet.to_string(),
                    searcher_wallet_address: transaction_data.accounts.searcher.to_string(),
                    fee_token: fee_token.to_string(),
                    referral_fee_ppm: transaction_data.data.referral_fee_ppm,
                    platform_fee_ppm: transaction_data.data.swap_platform_fee_ppm,
                    deadline: transaction_data.data.deadline,
                    token_program_user: transaction_data.accounts.token_program_user.to_string(),
                    token_program_searcher: transaction_data
                        .accounts
                        .token_program_searcher
                        .to_string(),

                    profile_id: bid.profile_id,
                };
                super::BidAnalytics::Swap(bid_analytics)
            }
        };
        self.db_analytics.add_bid(bid_analytics).await
    }
}
