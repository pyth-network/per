CREATE TABLE access_token
(
    token      VARCHAR(512) PRIMARY KEY,
    profile_id UUID         NOT NULL REFERENCES profile(id) ON DELETE CASCADE,
    revoked_at TIMESTAMP,
    created_at TIMESTAMP    NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP    NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TRIGGER update_updated_at
BEFORE UPDATE ON access_token
FOR EACH ROW
EXECUTE FUNCTION update_updated_at_column();
