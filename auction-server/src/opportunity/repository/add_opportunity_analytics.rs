use {
    super::{
        OpportunityAnalytics,
        OpportunityAnalyticsLimo,
        OpportunityAnalyticsSwap,
        OpportunityRemovalReason,
        Repository,
    },
    crate::{
        kernel::pyth_lazer::calculate_notional_value,
        opportunity::entities,
        state::Price,
    },
    base64::{
        engine::general_purpose,
        Engine,
    },
    solana_sdk::pubkey::Pubkey,
    std::collections::HashMap,
    time::OffsetDateTime,
};

impl Repository {
    pub async fn add_opportunity_analytics(
        &self,
        opportunity: entities::OpportunitySvm,
        removal_time: Option<OffsetDateTime>,
        removal_reason: Option<entities::OpportunityRemovalReason>,
        prices: HashMap<Pubkey, Price>,
        decimals: HashMap<Pubkey, u8>,
    ) -> anyhow::Result<()> {
        let sell_token = opportunity
            .sell_tokens
            .first()
            .ok_or(anyhow::anyhow!("Opportunity has no sell tokens"))?;
        let sell_token_notional_usd_value = calculate_notional_value(
            prices.get(&sell_token.token).cloned(),
            sell_token.amount,
            decimals.get(&sell_token.token).cloned(),
        );
        let buy_token = opportunity
            .buy_tokens
            .first()
            .ok_or(anyhow::anyhow!("Opportunity has no buy tokens"))?;
        let buy_token_notional_usd_value = calculate_notional_value(
            prices.get(&buy_token.token).cloned(),
            buy_token.amount,
            decimals.get(&buy_token.token).cloned(),
        );


        let removal_reason: Option<OpportunityRemovalReason> = removal_reason.map(|r| r.into());
        // NOTE: It's very easy to forget setting some field in one variant or the other.
        // We enforced this by destructing the params and make sure all the fields are used or explicitly discarded.
        // This way if we add a field to Limo or Swap variants later on, the code will not compile until we decide what we want to do with that field here.
        let opportunity_analytics = match opportunity.program.clone() {
            entities::OpportunitySvmProgram::Limo(entities::OpportunitySvmProgramLimo {
                order,
                order_address,
                slot,
            }) => OpportunityAnalytics::Limo(OpportunityAnalyticsLimo {
                id: opportunity.id,
                creation_time: opportunity.creation_time,
                permission_key: opportunity.permission_key.to_string(),
                chain_id: opportunity.chain_id.clone(),
                removal_time,
                removal_reason: removal_reason.map(|reason| {
                    serde_json::to_string(&reason).expect("Failed to serialize removal reason")
                }),
                sell_token_mint: sell_token.token.to_string(),
                sell_token_amount: sell_token.amount,
                sell_token_notional_usd_value,
                buy_token_mint: buy_token.token.to_string(),
                buy_token_amount: buy_token.amount,
                buy_token_notional_usd_value,

                order: general_purpose::STANDARD.encode(&order),
                order_address: order_address.to_string(),
                slot,

                profile_id: opportunity.profile_id,
            }),
            entities::OpportunitySvmProgram::Swap(entities::OpportunitySvmProgramSwap {
                user_wallet_address,
                user_mint_user_balance,
                fee_token,
                referral_fee_bps,
                referral_fee_ppm,
                platform_fee_bps,
                platform_fee_ppm,
                token_program_user,
                token_program_searcher,
                token_account_initialization_configs,
                memo,
                cancellable,
                minimum_lifetime,
                minimum_deadline: _,
            }) => OpportunityAnalytics::Swap(OpportunityAnalyticsSwap {
                id: opportunity.id,
                creation_time: opportunity.creation_time,
                permission_key: opportunity.permission_key.to_string(),
                chain_id: opportunity.chain_id.clone(),
                removal_time,
                removal_reason: removal_reason.map(|reason| {
                    serde_json::to_string(&reason).expect("Failed to serialize removal reason")
                }),
                searcher_token_mint: sell_token.token.to_string(),
                searcher_token_amount: sell_token.amount,
                searcher_token_notional_usd_value: sell_token_notional_usd_value,
                user_token_mint: buy_token.token.to_string(),
                user_token_amount: buy_token.amount,
                user_token_notional_usd_value: buy_token_notional_usd_value,

                user_wallet_address: user_wallet_address.to_string(),
                fee_token: serde_json::to_string(&fee_token)
                    .expect("Failed to serialize fee token"),
                referral_fee_bps,
                referral_fee_ppm,
                platform_fee_bps,
                platform_fee_ppm,
                token_program_user: token_program_user.to_string(),
                token_program_searcher: token_program_searcher.to_string(),
                user_mint_user_balance,
                token_account_initialization_configs: serde_json::to_string(
                    &token_account_initialization_configs,
                )
                .expect("Failed to serialize token account initialization configs"),
                memo,
                cancellable,
                minimum_lifetime,

                profile_id: opportunity.profile_id,
            }),
        };
        self.db_analytics
            .add_opportunity(opportunity_analytics)
            .await
    }
}
