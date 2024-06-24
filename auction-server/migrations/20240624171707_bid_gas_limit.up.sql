-- Add up migration script here
ALTER TABLE bid ADD COLUMN gas_limit NUMERIC(78, 0);
UPDATE bid SET gas_limit = 1000000000; -- indefinite gas limit
ALTER TABLE bid ALTER COLUMN gas_limit SET NOT NULL;
