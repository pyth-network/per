ALTER TABLE auction
DROP CONSTRAINT IF EXISTS auction_tx_hash_check,
DROP COLUMN chain_type;

ALTER TABLE auction ADD CONSTRAINT auction_tx_hash_check CHECK (LENGTH(tx_hash) = 32);
