CREATE TYPE chain_type AS ENUM ('evm', 'svm');

ALTER TABLE bid
ADD COLUMN metadata JSONB,
ADD COLUMN chain_type chain_type;

UPDATE bid SET chain_type = 'evm';

UPDATE bid SET metadata = jsonb_build_object(
    'target_contract', CONCAT('0x', encode(target_contract, 'hex')),
    'target_calldata', CONCAT('0x', encode(target_calldata, 'hex')),
    'bundle_index', bundle_index,
    'gas_limit', gas_limit
);

ALTER TABLE bid
DROP COLUMN target_contract,
DROP COLUMN target_calldata,
DROP COLUMN bundle_index,
DROP COLUMN gas_limit,
ALTER COLUMN chain_type SET NOT NULL,
ALTER COLUMN metadata SET NOT NULL;

-- Auction table --
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
