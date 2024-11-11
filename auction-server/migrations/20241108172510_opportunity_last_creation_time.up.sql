ALTER TABLE opportunity ADD COLUMN last_creation_time TIMESTAMP;
UPDATE opportunity SET last_creation_time = creation_time;
