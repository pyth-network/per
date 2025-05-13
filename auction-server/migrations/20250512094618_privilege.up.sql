CREATE TYPE privilege_state AS ENUM ('enabled', 'disabled');

CREATE TABLE privilege
(
    id         UUID             PRIMARY KEY,
    profile_id UUID             NOT NULL REFERENCES profile(id) ON DELETE CASCADE,
    feature    VARCHAR(255)     NOT NULL,
    state      privilege_state NOT NULL,
    created_at TIMESTAMP        NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP        NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TRIGGER update_updated_at
BEFORE UPDATE ON privilege
FOR EACH ROW
EXECUTE FUNCTION update_updated_at_column();
