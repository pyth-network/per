UPDATE bid
SET permission_key = BYTEA '\x00' || permission_key
WHERE chain_type = 'svm';

UPDATE auction
SET permission_key = BYTEA '\x00' || permission_key
WHERE chain_id = 'svm';

UPDATE opportunity
SET permission_key = BYTEA '\x00' || permission_key
WHERE chain_id = 'svm';
