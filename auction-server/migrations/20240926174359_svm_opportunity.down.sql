DELETE FROM opportunity WHERE chain_type = 'svm';
ALTER TABLE opportunity DROP COLUMN chain_type;

ALTER TABLE opportunity
ADD COLUMN target_contract BYTEA,
ADD COLUMN target_calldata BYTEA,
ADD COLUMN target_call_value NUMERIC(78, 0);

UPDATE opportunity
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

    target_call_value = (metadata->>'target_call_value')::NUMERIC(78, 0);

ALTER TABLE opportunity DROP COLUMN metadata;

ALTER TABLE opportunity
ALTER COLUMN target_contract SET NOT NULL,
ALTER COLUMN target_calldata SET NOT NULL,
ALTER COLUMN target_call_value SET NOT NULL;

ALTER TABLE opportunity ADD CONSTRAINT target_contract_length_check CHECK (LENGTH(target_contract) = 20);
