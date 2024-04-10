CREATE TABLE auction
(
    id              UUID PRIMARY KEY,
    creation_time   TIMESTAMP      NOT NULL,
    conclusion_time TIMESTAMP,
    permission_key  BYTEA          NOT NULL,
    chain_id        TEXT           NOT NULL,
    tx_hash         BYTEA          CHECK (LENGTH(tx_hash) = 32)
);
