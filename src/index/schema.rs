use anyhow::{Context, Result};
use std::sync::Arc;

/// Apply the code intelligence schema to index.db.
/// All DDL is IF NOT EXISTS so it is safe to call on every startup.
pub async fn ensure(conn: &Arc<turso::Connection>) -> Result<()> {
    // 1. Apply PRAGMAs using the safe pragma_update() method.
    // GOTCHA: Common SQLite pragmas like 'journal_mode' return a row containing the new value.
    // In the Turso Rust driver, .execute() and .execute_batch() strictly expect 0 rows and
    // will fail with "unexpected row during execution" if rows are returned.
    conn.pragma_update("journal_mode", "WAL")
        .await
        .context("Failed to set journal_mode=WAL")?;
    conn.pragma_update("busy_timeout", "5000")
        .await
        .context("Failed to set busy_timeout")?;

    // 2. Apply base schema.
    // GOTCHA: In Turso/Limbo 0.6.0-pre.22, 'IF NOT EXISTS' can be unreliable when multiple
    // DDL statements are run in a single execute_batch() call, sometimes failing with
    // "already exists" even if the table exists. We run them individually for stability.
    //
    // GOTCHA: Limbo's parser does not yet support 'WITHOUT ROWID' tables. SQLite's FTS5
    // extension uses WITHOUT ROWID for its internal shadow tables, making FTS5 databases
    // unreadable by Limbo. We switch to Turso's native 'USING fts' which is Limbo-compatible.
    // Keys must match the actual DB object name (table or index name) because we
    // query sqlite_schema by name to check existence before attempting DDL.
    // GOTCHA: Limbo can return a false "already exists" error for CREATE TABLE IF NOT
    // EXISTS even when the table does not exist. Catching that error would silently
    // skip creation; a downstream CREATE INDEX then fails with "table does not exist".
    // Pre-checking sqlite_schema avoids the false-positive swallow entirely.
    let ddl = [
        (
            "index_files",
            "CREATE TABLE IF NOT EXISTS index_files (
             path        TEXT PRIMARY KEY,
             content_hash TEXT NOT NULL,
             indexed_at  TEXT NOT NULL
         )",
        ),
        (
            "index_chunks",
            "CREATE TABLE IF NOT EXISTS index_chunks (
             id             TEXT PRIMARY KEY,
             file_path      TEXT NOT NULL,
             symbol_name    TEXT NOT NULL,
             qualified_name TEXT NOT NULL,
             symbol_kind    TEXT NOT NULL,
             signature      TEXT,
             doc_comment    TEXT,
             body_preview   TEXT,
             line_start     INTEGER NOT NULL,
             line_end       INTEGER NOT NULL,
             language       TEXT NOT NULL,
             churn_count    INTEGER DEFAULT 0,
             hotspot_score  REAL DEFAULT 0.0,
             indexed_at     TEXT NOT NULL,
             content_hash   TEXT NOT NULL
         )",
        ),
        (
            "index_chunks_file",
            "CREATE INDEX IF NOT EXISTS index_chunks_file ON index_chunks (file_path)",
        ),
        (
            "index_vectors",
            "CREATE TABLE IF NOT EXISTS index_vectors (
             chunk_id    TEXT NOT NULL REFERENCES index_chunks(id) ON DELETE CASCADE,
             embed_model TEXT NOT NULL,
             embedding   BLOB NOT NULL,
             PRIMARY KEY (chunk_id, embed_model)
         )",
        ),
        (
            "index_chunks_fts",
            "CREATE INDEX IF NOT EXISTS index_chunks_fts ON index_chunks USING fts(symbol_name, qualified_name, signature, doc_comment, body_preview)",
        ),
        (
            "index_commits",
            "CREATE TABLE IF NOT EXISTS index_commits (
             sha       TEXT PRIMARY KEY,
             message   TEXT NOT NULL,
             author    TEXT,
             timestamp TEXT
         )",
        ),
        (
            "index_chunk_commits",
            "CREATE TABLE IF NOT EXISTS index_chunk_commits (
             chunk_id   TEXT NOT NULL REFERENCES index_chunks(id) ON DELETE CASCADE,
             commit_sha TEXT NOT NULL REFERENCES index_commits(sha) ON DELETE CASCADE,
             PRIMARY KEY (chunk_id, commit_sha)
         )",
        ),
        (
            "index_chunk_commits_by_sha",
            "CREATE INDEX IF NOT EXISTS index_chunk_commits_by_sha ON index_chunk_commits (commit_sha)",
        ),
    ];

    for (name, stmt) in ddl {
        // Check sqlite_schema first to avoid Limbo's false "already exists" bug.
        let already_exists: bool = {
            let mut rows = conn
                .query(
                    "SELECT count(*) FROM sqlite_schema WHERE name = ?1",
                    (name.to_string(),),
                )
                .await
                .unwrap_or_else(|_| unreachable!());
            rows.next()
                .await
                .ok()
                .flatten()
                .and_then(|r| r.get::<i64>(0).ok())
                .unwrap_or(0)
                > 0
        };
        if already_exists {
            tracing::trace!(ddl = %name, "index schema item already exists");
            continue;
        }
        conn.execute(stmt, ())
            .await
            .context(format!("Failed to execute DDL for {}: {}", name, stmt))?;
        tracing::trace!(ddl = %name, "index schema item created");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use turso::Builder;

    #[tokio::test]
    async fn test_schema_idempotency() -> Result<()> {
        let db = Builder::new_local(":memory:")
            .experimental_index_method(true)
            .build()
            .await?;
        let conn = Arc::new(db.connect()?);

        // First run
        ensure(&conn).await.context("First schema run failed")?;

        // Second run should succeed (idempotency check)
        ensure(&conn)
            .await
            .context("Second schema run failed (idempotency issue)")?;

        Ok(())
    }
}
