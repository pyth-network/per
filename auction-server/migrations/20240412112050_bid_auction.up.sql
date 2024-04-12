ALTER TABLE bid ADD CONSTRAINT fk_auction_id
FOREIGN KEY (auction_id)
REFERENCES auction(id)
ON DELETE RESTRICT;

ALTER TABLE bid DROP COLUMN removal_time;
ALTER TABLE bid ADD COLUMN bundle_index INTEGER CHECK (bundle_index >= 0);
