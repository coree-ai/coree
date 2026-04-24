use anyhow::{Context, Result};
use std::sync::Arc;

/// Apply the code intelligence schema to index.db.
/// All DDL is IF NOT EXISTS so it is safe to call on every startup.
pub async fn ensure(conn: &Arc<turso::Connection>) -> Result<()> {
    // 1. Apply PRAGMAs using the safe pragma_update() method.
    conn.pragma_update("journal_mode", "WAL")
        .await
        .context("Failed to set journal_mode=WAL")?;
    conn.pragma_update("busy_timeout", "5000")
        .await
        .context("Failed to set busy_timeout")?;

    // 2. Apply base schema and native FTS index.
    // Limbo (turso) uses native USING fts instead of the C-based FTS5 module.
    // Native FTS indexes automatically stay in sync; no triggers required.
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS index_files (
             path        TEXT PRIMARY KEY,
             content_hash TEXT NOT NULL,
             indexed_at  TEXT NOT NULL
         );

         CREATE TABLE IF NOT EXISTS index_chunks (
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
         );

         CREATE INDEX IF NOT EXISTS index_chunks_file
             ON index_chunks (file_path);

         CREATE TABLE IF NOT EXISTS index_vectors (
             chunk_id    TEXT NOT NULL REFERENCES index_chunks(id) ON DELETE CASCADE,
             embed_model TEXT NOT NULL,
             embedding   BLOB NOT NULL,
             PRIMARY KEY (chunk_id, embed_model)
         );

         -- Turso native FTS index
         CREATE INDEX IF NOT EXISTS index_chunks_fts 
             ON index_chunks USING fts(symbol_name, qualified_name, signature, doc_comment, body_preview);

         CREATE TABLE IF NOT EXISTS index_commits (
             sha       TEXT PRIMARY KEY,
             message   TEXT NOT NULL,
             author    TEXT,
             timestamp TEXT
         );

         CREATE TABLE IF NOT EXISTS index_chunk_commits (
             chunk_id   TEXT NOT NULL REFERENCES index_chunks(id) ON DELETE CASCADE,
             commit_sha TEXT NOT NULL REFERENCES index_commits(sha) ON DELETE CASCADE,
             PRIMARY KEY (chunk_id, commit_sha)
         );

         CREATE INDEX IF NOT EXISTS index_chunk_commits_by_sha
             ON index_chunk_commits (commit_sha);
        ",
    ).await?;

    Ok(())
}
