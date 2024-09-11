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
