CREATE TABLE auction
(
    id              UUID PRIMARY KEY,
    conclusion_time TIMESTAMP      NOT NULL,
    permission_key  BYTEA          NOT NULL,
    chain_id        TEXT           NOT NULL,
    tx_hash         BYTEA          NOT NULL CHECK (LENGTH(tx_hash) = 32)
);
