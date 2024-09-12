DELETE FROM bid WHERE chain_type = 'svm';
DELETE FROM auction WHERE chain_type = 'svm';
ALTER TABLE bid DROP COLUMN chain_type;
ALTER TABLE auction DROP COLUMN chain_type;
DROP TYPE chain_type;

ALTER TABLE bid
ADD COLUMN target_contract BYTEA,
ADD COLUMN target_calldata BYTEA,
ADD COLUMN bundle_index INTEGER CHECK (bundle_index >= 0),
ADD COLUMN gas_limit NUMERIC(78, 0);

UPDATE bid
SET target_contract = decode(
    CASE
        WHEN metadata->>'target_contract' LIKE '0x%'
        THEN SUBSTRING(metadata->>'target_contract' FROM 3)
        ELSE metadata->>'target_contract'
    END, 'hex')::BYTEA,

    target_calldata = decode(
    CASE
        WHEN metadata->>'target_calldata' LIKE '0x%'
        THEN SUBSTRING(metadata->>'target_calldata' FROM 3)
        ELSE metadata->>'target_calldata'
    END, 'hex')::BYTEA,

    bundle_index = (metadata->>'bundle_index')::INTEGER,
    gas_limit = (metadata->>'gas_limit')::NUMERIC(78, 0);

ALTER TABLE bid DROP COLUMN metadata;

ALTER TABLE bid
ALTER COLUMN target_contract SET NOT NULL,
ALTER COLUMN target_calldata SET NOT NULL,
ALTER COLUMN gas_limit SET NOT NULL;

ALTER TABLE bid ADD CONSTRAINT target_contract_length_check CHECK (LENGTH(target_contract) = 20);

ALTER TABLE auction DROP CONSTRAINT IF EXISTS auction_tx_hash_check;
ALTER TABLE auction ADD CONSTRAINT auction_tx_hash_check CHECK (LENGTH(tx_hash) = 32);
