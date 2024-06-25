CREATE INDEX opportunity_permission_key_creation_time_idx ON opportunity (chain_id, permission_key, creation_time);
CREATE INDEX opportunity_creation_time_idx ON opportunity (chain_id, creation_time);
