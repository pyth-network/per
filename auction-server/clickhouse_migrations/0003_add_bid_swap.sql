CREATE TABLE IF NOT EXISTS bid_swap (
    id              UUID,
    creation_time   DateTime64(6),
    initiation_time DateTime64(6),
    permission_key  String,
    chain_id        LowCardinality(String),
    transaction     String,
    bid_amount      UInt64,

    auction_id      Nullable(UUID),
    opportunity_id  Nullable(UUID),
    conclusion_time Nullable(DateTime64(6)),

    -- Token info
    searcher_token_mint      String,
    searcher_token_amount    UInt64,
    searcher_token_usd_price Nullable(Float64),

    user_token_mint      String,
    user_token_amount    UInt64,
    user_token_usd_price Nullable(Float64),

    status        LowCardinality(String),
    status_reason Nullable(String),

    -- Profile
    profile_id       Nullable(UUID),

    user_wallet_address     String,
    searcher_wallet_address String,
    fee_token               String,
    referral_fee_ppm        UInt64,
    platform_fee_ppm        UInt64,
    deadline                Int64,
    token_program_user      String,
    token_program_searcher  String,
) ENGINE = ReplacingMergeTree
ORDER BY (creation_time, id);
