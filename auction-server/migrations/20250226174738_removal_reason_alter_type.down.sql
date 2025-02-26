UPDATE opportunity SET removal_reason = NULL WHERE removal_reason = 'server_restart';
CREATE TYPE temp_opportunity_removal_reason AS ENUM ('expired', 'invalid');
ALTER TABLE opportunity
    ALTER COLUMN removal_reason TYPE temp_opportunity_removal_reason
    USING removal_reason::text::temp_opportunity_removal_reason;
DROP TYPE IF EXISTS opportunity_removal_reason;
ALTER TYPE temp_opportunity_removal_reason RENAME TO opportunity_removal_reason;
