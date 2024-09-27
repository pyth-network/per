DELETE FROM profile WHERE role = 'protocol';
ALTER TABLE profile DROP COLUMN role;

DROP TYPE profile_role;
