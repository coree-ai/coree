-- Recreate memories_topic_key as a partial unique index that only enforces
-- uniqueness among active rows. This allows a deleted memory's topic_key to
-- be reused by a new active memory (coree-ai/coree#31, 1-H2).
DROP INDEX IF EXISTS memories_topic_key;
CREATE UNIQUE INDEX IF NOT EXISTS memories_topic_key
    ON memories (project_id, topic_key)
    WHERE topic_key IS NOT NULL AND status = 'active';
