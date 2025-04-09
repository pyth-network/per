UPDATE bid SET status = 'cancelled' WHERE status = 'submission_failed_cancelled';
UPDATE bid SET status = 'expired' WHERE status = 'submission_failed_deadline_passed';
CREATE TYPE temp_bid_status AS ENUM ('pending', 'lost', 'submitted', 'won', 'expired', 'failed', 'cancelled', 'awaiting_signature', 'sent_to_user_for_submission');
ALTER TABLE bid
    ALTER COLUMN status TYPE temp_bid_status
    USING status::text::temp_bid_status;
DROP TYPE IF EXISTS bid_status;
ALTER TYPE temp_bid_status RENAME TO bid_status;
