-- Add nullable git provenance columns.
-- git_ref is the branch name (from `git symbolic-ref --short HEAD`); NULL when detached HEAD.
-- git_author is the git user identity (from `git config user.name`); NULL when unavailable.
-- Both are server-derived at write time, never agent-supplied, return-only (never queried).
-- Idempotency: runner catches "duplicate column name" error and skips.
ALTER TABLE memories ADD COLUMN git_ref TEXT;
ALTER TABLE memories ADD COLUMN git_author TEXT;
