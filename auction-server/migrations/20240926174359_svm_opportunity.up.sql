ALTER TABLE opportunity
ADD COLUMN metadata JSONB,
ADD COLUMN chain_type chain_type;

UPDATE opportunity SET chain_type = 'evm';

UPDATE opportunity SET metadata = jsonb_build_object(
    'target_contract', CONCAT('0x', encode(target_contract, 'hex')),
    'target_calldata', CONCAT('0x', encode(target_calldata, 'hex')),
    -- The 'target_call_value' is being converted to a character string using the TO_CHAR function
    -- with a format model that removes any leading or trailing spaces (FM) and enforces a specific
    -- number format. This ensures that the number is displayed in a compact, formatted way without unnecessary padding.
    'target_call_value', TO_CHAR(target_call_value, 'FM999999999999999999999999999999999999999999999999999999999999999999999999999999')
);

ALTER TABLE opportunity
DROP COLUMN target_contract,
DROP COLUMN target_calldata,
DROP COLUMN target_call_value,
ALTER COLUMN chain_type SET NOT NULL,
ALTER COLUMN metadata SET NOT NULL;
