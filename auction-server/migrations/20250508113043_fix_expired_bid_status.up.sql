UPDATE bid
SET status = 'lost'
WHERE status = 'expired'
  AND auction_id IS NULL;
