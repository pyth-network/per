CREATE TYPE permission_state AS ENUM ('enabled', 'disabled');

CREATE TABLE permission
(
    id         UUID             PRIMARY KEY,
    profile_id UUID             NOT NULL REFERENCES profile(id) ON DELETE CASCADE,
    feature    VARCHAR(255)     NOT NULL,
    state      permission_state NOT NULL,
    created_at TIMESTAMP        NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP        NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TRIGGER update_updated_at
BEFORE UPDATE ON permission
FOR EACH ROW
EXECUTE FUNCTION update_updated_at_column();
