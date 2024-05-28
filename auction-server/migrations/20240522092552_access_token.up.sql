CREATE TABLE access_token
(
    id         UUID         PRIMARY KEY,
    token      VARCHAR(512) UNIQUE NOT NULL,
    profile_id UUID         NOT NULL REFERENCES profile(id) ON DELETE CASCADE,
    revoked_at TIMESTAMP,
    created_at TIMESTAMP    NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP    NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TRIGGER update_updated_at
BEFORE UPDATE ON access_token
FOR EACH ROW
EXECUTE FUNCTION update_updated_at_column();
