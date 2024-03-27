CREATE TABLE opportunity
(
    id                UUID PRIMARY KEY,
    creation_time     TIMESTAMP      NOT NULL,
    permission_key    BYTEA          NOT NULL,
    chain_id          TEXT           NOT NULL,
    target_contract   BYTEA          NOT NULL CHECK (LENGTH(target_contract) = 20),
    target_call_value NUMERIC(78, 0) NOT NULL,
    target_calldata   BYTEA          NOT NULL,
    sell_tokens       JSONB          NOT NULL,
    buy_tokens        JSONB          NOT NULL,
    removal_time      TIMESTAMP
);
