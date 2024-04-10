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
    auction_id      UUID           REFERENCES auction(id) ON DELETE RESTRICT,
    bundle_index    INTEGER        CHECK (bundle_index >= 0)
);
