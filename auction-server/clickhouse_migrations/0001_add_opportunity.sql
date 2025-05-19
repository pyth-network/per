CREATE TABLE IF NOT EXISTS opportunity (
    id UUID,
    creation_time DateTime64(6),
    permission_key String,
    chain_id LowCardinality(String),

    program LowCardinality(String),

    -- Token info
    sell_token_mint String,
    sell_token_amount UInt64,
    sell_token_usd_price Nullable(Float64),

    buy_token_mint String,
    buy_token_amount UInt64,
    buy_token_usd_price Nullable(Float64),

    -- Optional removal tracking
    removal_time Nullable(DateTime64(6)),
    removal_reason Nullable(String),

    -- For Limo variant
    limo_order Nullable(String),
    limo_order_address Nullable(String),
    limo_slot Nullable(UInt64),

    -- For Swap variant
    swap_user_wallet_address Nullable(String),
    swap_fee_token Nullable(String),
    swap_referral_fee_bps Nullable(UInt16),
    swap_referral_fee_ppm Nullable(UInt64),
    swap_platform_fee_bps Nullable(UInt64),
    swap_platform_fee_ppm Nullable(UInt64),
    swap_token_program_user Nullable(String),
    swap_token_program_searcher Nullable(String),
    swap_token_account_initialization_configs Nullable(String),
    swap_user_mint_user_balance Nullable(UInt64),
    swap_memo Nullable(String),
    swap_cancellable Nullable(Bool),
    swap_minimum_lifetime Nullable(UInt32),

    -- Profile
    profile_id Nullable(UUID),

) ENGINE = ReplacingMergeTree
ORDER BY (creation_time, id);
