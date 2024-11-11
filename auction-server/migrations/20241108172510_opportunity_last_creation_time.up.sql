ALTER TABLE opportunity ADD COLUMN last_creation_time TIMESTAMP NOT NULL DEFAULT NOW();
ALTER TABLE opportunity ALTER COLUMN last_creation_time DROP DEFAULT;
UPDATE opportunity SET last_creation_time = creation_time;
