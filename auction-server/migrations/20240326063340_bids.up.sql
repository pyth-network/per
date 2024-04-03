CREATE TYPE bid_status AS ENUM ('pending', 'lost', 'submitted');

CREATE TABLE bid
(
    id              UUID PRIMARY KEY,
    creation_time   TIMESTAMP      NOT NULL,
    permission_key  BYTEA          NOT NULL,
    chain_id        TEXT           NOT NULL,
    target_contract BYTEA          NOT NULL CHECK (LENGTH(target_contract) = 20),
    target_calldata BYTEA          NOT NULL,
    bid_amount      NUMERIC(78, 0) NOT NULL,
    status          bid_status     NOT NULL,
    tx_hash         BYTEA          CHECK (LENGTH(tx_hash) = 32),
    auction_id      UUID,     -- TODO: should be linked to the auction table in the future
    removal_time    TIMESTAMP -- TODO: should be removed and read from the auction table in the future
);
