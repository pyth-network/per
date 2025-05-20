CREATE TABLE IF NOT EXISTS opportunity_limo (
    id UUID,
    creation_time DateTime64(6),
    permission_key String,
    chain_id LowCardinality(String),

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

    -- Profile
    profile_id Nullable(UUID),

    -- For Limo variant
    order String,
    order_address String,
    slot UInt64
) ENGINE = ReplacingMergeTree
ORDER BY (creation_time, id);
