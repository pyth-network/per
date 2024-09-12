ALTER TABLE auction
DROP CONSTRAINT IF EXISTS auction_tx_hash_check;

ALTER TABLE auction ADD COLUMN chain_type chain_type;
UPDATE auction SET chain_type = 'evm';
ALTER TABLE auction ALTER COLUMN chain_type SET NOT NULL;

ALTER TABLE auction
ADD CONSTRAINT auction_tx_hash_check
CHECK (
    (chain_type = 'evm' AND LENGTH(tx_hash) = 32) OR
    (chain_type = 'svm' AND LENGTH(tx_hash) = 64)
);
