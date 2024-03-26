CREATE TABLE bid
(
    id              UUID PRIMARY KEY,
    creation_time   TIMESTAMP      NOT NULL,
    permission_key  BYTEA          NOT NULL,
    chain_id        TEXT           NOT NULL,
    target_contract BYTEA          NOT NULL,
    target_calldata BYTEA          NOT NULL,
    bid_amount      NUMERIC(80, 0) NOT NULL,
    status          TEXT           NOT NULL, -- pending, lost, submitted
    auction_id      UUID, -- TODO: should be linked to the auction table in the future
    removal_time    TIMESTAMP -- TODO: should be removed and read from the auction table in the future
);
