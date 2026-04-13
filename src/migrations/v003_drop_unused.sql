-- Drop sessions table and unused columns (confidence, supersedes).
-- DROP TABLE IF EXISTS is safe to re-run.
-- ALTER TABLE DROP COLUMN idempotency: runner catches "no such column" error and skips.
DROP TABLE IF EXISTS sessions;
ALTER TABLE memories DROP COLUMN confidence;
ALTER TABLE memories DROP COLUMN supersedes;
