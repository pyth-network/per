UPDATE bid
SET permission_key = SUBSTRING(permission_key FROM 2)
WHERE chain_type = 'svm';

UPDATE auction
SET permission_key = SUBSTRING(permission_key FROM 2)
WHERE chain_id = 'svm';

UPDATE opportunity
SET permission_key = SUBSTRING(permission_key FROM 2)
WHERE chain_id = 'svm';
