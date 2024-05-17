UPDATE bid SET status = 'lost' WHERE status = 'simulation_failed';
CREATE TYPE temp_bid_status AS ENUM ('pending', 'lost', 'submitted', 'won');
ALTER TABLE bid
    ALTER COLUMN status TYPE temp_bid_status
    USING status::text::temp_bid_status;
DROP TYPE IF EXISTS bid_status;
ALTER TYPE temp_bid_status RENAME TO bid_status;
