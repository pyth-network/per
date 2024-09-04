CREATE TYPE opportunity_removal_reason AS ENUM ('expired', 'invalid');
ALTER TABLE opportunity ADD COLUMN removal_reason opportunity_removal_reason;
