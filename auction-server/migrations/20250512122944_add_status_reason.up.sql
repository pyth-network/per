CREATE TYPE status_reason AS ENUM ('insufficient_user_funds', 'insufficient_searcher_funds', 'insufficient_funds_sol_transfer', 'deadline_passed', 'other');
ALTER TABLE bid ADD COLUMN status_reason status_reason;
