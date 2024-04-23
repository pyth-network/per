UPDATE bid SET status = 'pending' WHERE status = 'final_simulation_failed';
CREATE TYPE new_bid_status AS ENUM ('pending', 'lost', 'submitted');
ALTER TABLE bid
    ALTER COLUMN status TYPE new_bid_status
    USING status::text::new_bid_status;
DROP TYPE IF EXISTS bid_status;
ALTER TYPE new_bid_status RENAME TO bid_status;
