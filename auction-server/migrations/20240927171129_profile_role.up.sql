CREATE TYPE profile_role AS ENUM ('searcher', 'protocol');

ALTER TABLE profile
ADD COLUMN role profile_role;

UPDATE profile SET role = 'searcher';

ALTER TABLE profile
ALTER COLUMN role SET NOT NULL;
