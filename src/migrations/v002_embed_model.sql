-- Add embed_model column to memory_vectors.
-- Idempotency: runner catches "duplicate column name" error and skips.
ALTER TABLE memory_vectors ADD COLUMN embed_model TEXT NOT NULL DEFAULT '';
