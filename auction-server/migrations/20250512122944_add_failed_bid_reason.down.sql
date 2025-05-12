-- Add down migration script here
ALTER TABLE bid DROP COLUMN reason;
DROP TYPE bid_failed_reason;
