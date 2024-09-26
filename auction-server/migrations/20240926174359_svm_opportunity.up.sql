ALTER TABLE opportunity
ADD COLUMN metadata JSONB,
ADD COLUMN chain_type chain_type;

UPDATE opportunity SET chain_type = 'evm';

UPDATE opportunity SET metadata = jsonb_build_object(
    'target_contract', CONCAT('0x', encode(target_contract, 'hex')),
    'target_calldata', CONCAT('0x', encode(target_calldata, 'hex')),
    'target_call_value', TO_CHAR(target_call_value, 'FM999999999999999999999999999999999999999999999999999999999999999999999999999999')
);

ALTER TABLE opportunity
DROP COLUMN target_contract,
DROP COLUMN target_calldata,
DROP COLUMN target_call_value,
ALTER COLUMN chain_type SET NOT NULL,
ALTER COLUMN metadata SET NOT NULL;
