ALTER TABLE bid DROP COLUMN bundle_index;
ALTER TABLE bid ADD COLUMN removal_time TIMESTAMP;
ALTER TABLE bid DROP CONSTRAINT fk_auction_id;
