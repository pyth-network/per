CREATE TABLE IF NOT EXISTS bid_limo (
    id              UUID,
    creation_time   DateTime64(6),
    initiation_time DateTime64(6),
    permission_key  String,
    chain_id        LowCardinality(String),
    transaction     String,
    bid_amount      UInt64,

    auction_id      Nullable(UUID),
    conclusion_time Nullable(DateTime64(6)),

    status        LowCardinality(String),

    -- Profile
    profile_id       Nullable(UUID),

    deadline           Int64,
    router             String,
    permission_account String,
) ENGINE = ReplacingMergeTree
ORDER BY (creation_time, id);
