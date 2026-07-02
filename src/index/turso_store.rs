use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use super::git::CommitInfo;
use super::search::{self, CodeResult};
use super::store::{CodeIndexStore, IndexedFile};
use crate::embed;

pub struct TursoStore {
    db: turso::Database,
    conn: Arc<turso::Connection>,
    git_history: bool,
}

impl TursoStore {
    pub fn new(db: turso::Database, conn: Arc<turso::Connection>, git_history: bool) -> Self {
        Self { db, conn, git_history }
    }

    /// Apply the code intelligence schema. All DDL is IF NOT EXISTS so it is safe
    /// to call on every startup. This is a free function so main.rs::reset_stored_version
    /// can call it without a TursoStore.
    pub async fn ensure_schema(conn: &Arc<turso::Connection>) -> Result<()> {
        conn.pragma_update("journal_mode", "WAL")
            .await
            .context("Failed to set journal_mode=WAL")?;
        conn.pragma_update("busy_timeout", "5000")
            .await
            .context("Failed to set busy_timeout")?;

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
            (
                "index_file_coupling",
                "CREATE TABLE IF NOT EXISTS index_file_coupling (
                 file_a         TEXT NOT NULL,
                 file_b         TEXT NOT NULL,
                 shared_commits INTEGER NOT NULL DEFAULT 0,
                 last_shared_ts TEXT NOT NULL,
                 PRIMARY KEY (file_a, file_b)
             )",
            ),
            (
                "meta",
                "CREATE TABLE IF NOT EXISTS meta (
                 key   TEXT PRIMARY KEY,
                 value TEXT NOT NULL
             )",
            ),
        ];

        for (name, stmt) in ddl {
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

    fn new_write_conn(&self) -> Result<Arc<turso::Connection>> {
        Ok(Arc::new(
            self.db
                .connect()
                .context("Failed to create index connection")?,
        ))
    }
}

#[async_trait]
impl CodeIndexStore for TursoStore {
    async fn stored_logic_version(&self) -> Result<Option<u32>> {
        let mut rows = self
            .conn
            .query(
                "SELECT value FROM meta WHERE key = 'index_logic_version'",
                (),
            )
            .await?;
        if let Some(row) = rows.next().await? {
            let v: String = row.get(0)?;
            v.parse::<u32>().ok().map(Some).ok_or_else(|| {
                anyhow::anyhow!("Invalid index_logic_version value in meta table: {v}")
            })
        } else {
            Ok(None)
        }
    }

    async fn set_stored_logic_version(&self, version: u32) -> Result<()> {
        let conn = self.new_write_conn()?;
        conn.execute(
            "INSERT OR REPLACE INTO meta (key, value) VALUES ('index_logic_version', ?1)",
            (version.to_string(),),
        )
        .await?;
        Ok(())
    }

    async fn clear_all(&self) -> Result<()> {
        let conn = self.new_write_conn()?;
        conn.execute("DELETE FROM index_vectors", ()).await?;
        conn.execute("DELETE FROM index_chunk_commits", ()).await?;
        conn.execute("DELETE FROM index_chunks", ()).await?;
        conn.execute("DELETE FROM index_files", ()).await?;
        conn.execute("DELETE FROM index_commits", ()).await?;
        conn.execute("DELETE FROM index_file_coupling", ()).await?;
        Ok(())
    }

    async fn file_content_hash(&self, rel_path: &str) -> Result<Option<String>> {
        let mut rows = self
            .conn
            .query(
                "SELECT content_hash FROM index_files WHERE path = ?1",
                (rel_path.to_string(),),
            )
            .await?;
        Ok(rows.next().await?.map(|r| r.get(0)).transpose()?)
    }

    async fn replace_file(&self, file: IndexedFile) -> Result<()> {
        let conn = self.new_write_conn()?;

        conn.execute(
            "DELETE FROM index_vectors WHERE chunk_id IN (SELECT id FROM index_chunks WHERE file_path = ?1)",
            (file.rel_path.clone(),),
        )
        .await?;
        conn.execute(
            "DELETE FROM index_chunk_commits WHERE chunk_id IN (SELECT id FROM index_chunks WHERE file_path = ?1)",
            (file.rel_path.clone(),),
        )
        .await?;
        conn.execute(
            "DELETE FROM index_chunks WHERE file_path = ?1",
            (file.rel_path.clone(),),
        )
        .await?;

        let now = Utc::now().to_rfc3339();
        let model_id = crate::embed::model_id();

        for commit in &file.commits {
            let timestamp = chrono::DateTime::from_timestamp(commit.timestamp_unix, 0)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_default();
            let _ = conn
                .execute(
                    "INSERT OR IGNORE INTO index_commits (sha, message, author, timestamp) \
                     VALUES (?1, ?2, ?3, ?4)",
                    (
                        commit.sha.clone(),
                        commit.message.clone(),
                        commit.author.clone(),
                        timestamp,
                    ),
                )
                .await;
        }

        for ic in &file.chunks {
            let blob = embed::floats_to_blob(&ic.embedding);
            let chunk_id = Uuid::new_v4().to_string();

            conn.execute(
                "INSERT INTO index_chunks \
                 (id, file_path, symbol_name, qualified_name, symbol_kind, signature, \
                  doc_comment, body_preview, line_start, line_end, language, \
                  churn_count, hotspot_score, indexed_at, content_hash) \
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
                (
                    chunk_id.clone(),
                    file.rel_path.clone(),
                    ic.chunk.symbol_name.clone(),
                    ic.chunk.qualified_name.clone(),
                    ic.chunk.symbol_kind.clone(),
                    ic.chunk.signature.clone(),
                    ic.chunk.doc_comment.clone(),
                    ic.chunk.body_preview.clone(),
                    ic.chunk.line_start as i64,
                    ic.chunk.line_end as i64,
                    file.language.clone(),
                    file.churn_count,
                    file.hotspot_score,
                    now.clone(),
                    ic.content_hash.clone(),
                ),
            )
            .await?;

            conn.execute(
                "INSERT OR REPLACE INTO index_vectors (chunk_id, embed_model, embedding) \
                 VALUES (?1, ?2, ?3)",
                (chunk_id.clone(), model_id.clone(), blob),
            )
            .await?;

            for commit in &file.commits {
                let _ = conn
                    .execute(
                        "INSERT OR IGNORE INTO index_chunk_commits (chunk_id, commit_sha) \
                     VALUES (?1, ?2)",
                        (chunk_id.clone(), commit.sha.clone()),
                    )
                    .await;
            }
        }

        // Refresh file coupling
        if self.git_history {
            conn.execute(
                "DELETE FROM index_file_coupling WHERE file_a = ?1 OR file_b = ?1",
                (file.rel_path.clone(),),
            )
            .await?;

            conn.execute(
                "INSERT OR REPLACE INTO index_file_coupling (file_a, file_b, shared_commits, last_shared_ts)
             SELECT
               CASE WHEN ic1.file_path < ic2.file_path THEN ic1.file_path ELSE ic2.file_path END,
               CASE WHEN ic1.file_path < ic2.file_path THEN ic2.file_path ELSE ic1.file_path END,
               COUNT(DISTINCT icc1.commit_sha),
               MAX(c.timestamp)
             FROM index_chunk_commits icc1
             JOIN index_chunks ic1 ON ic1.id = icc1.chunk_id
             JOIN index_chunk_commits icc2 ON icc2.commit_sha = icc1.commit_sha
             JOIN index_chunks ic2 ON ic2.id = icc2.chunk_id AND ic2.file_path != ic1.file_path
             JOIN index_commits c ON c.sha = icc1.commit_sha
             WHERE ic1.file_path = ?1
               AND icc1.commit_sha NOT IN (
                   SELECT cc.commit_sha FROM index_chunk_commits cc
                   JOIN index_chunks ic ON ic.id = cc.chunk_id
                   GROUP BY cc.commit_sha
                   HAVING COUNT(DISTINCT ic.file_path) > 25
               )
             GROUP BY 1, 2
             HAVING COUNT(DISTINCT icc1.commit_sha) >= 2",
                (file.rel_path.clone(),),
            )
            .await?;
        }

        // Upsert file hash
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT OR REPLACE INTO index_files (path, content_hash, indexed_at) \
             VALUES (?1, ?2, ?3)",
            (file.rel_path, file.content_hash.clone(), now),
        )
        .await?;

        Ok(())
    }

    async fn remove_file(&self, rel_path: &str) -> Result<()> {
        let conn = self.new_write_conn()?;
        conn.execute(
            "DELETE FROM index_vectors WHERE chunk_id IN (SELECT id FROM index_chunks WHERE file_path = ?1)",
            (rel_path.to_string(),),
        )
        .await?;
        conn.execute(
            "DELETE FROM index_chunk_commits WHERE chunk_id IN (SELECT id FROM index_chunks WHERE file_path = ?1)",
            (rel_path.to_string(),),
        )
        .await?;
        conn.execute(
            "DELETE FROM index_chunks WHERE file_path = ?1",
            (rel_path.to_string(),),
        )
        .await?;
        conn.execute("DELETE FROM index_files WHERE path = ?1", (rel_path.to_string(),))
            .await?;
        Ok(())
    }

    async fn search_code(
        &self,
        embedding: Vec<f32>,
        query: &str,
        limit: usize,
    ) -> Result<Vec<CodeResult>> {
        let blob = embed::floats_to_blob(&embedding);
        let model = embed::model_id();
        let k = limit * 2;

        // Stream A: vector search
        let mut vector_ranks: HashMap<String, usize> = HashMap::new();
        match self
            .conn
            .query(
                "SELECT ic.id, vector_distance_cos(iv.embedding, vector32(?1)) as dist
             FROM index_chunks ic
             JOIN index_vectors iv ON iv.chunk_id = ic.id
             WHERE iv.embed_model = ?2
             ORDER BY dist
             LIMIT ?3",
                (blob, model, k as i64),
            )
            .await
        {
            Ok(mut rows) => {
                let mut rank = 0usize;
                while let Some(row) = rows.next().await? {
                    if let Ok(id) = row.get::<String>(0) {
                        vector_ranks.insert(id, rank);
                        rank += 1;
                    }
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "code vector search failed, falling back to FTS-only");
            }
        }

        // Stream B: FTS
        let fts_ranks: HashMap<String, usize> = {
            let fts_q = build_fts_query(query);
            if fts_q.is_empty() {
                HashMap::new()
            } else {
                let mut rows = self.conn.query(
                    "SELECT id
                     FROM index_chunks
                     WHERE fts_match(symbol_name, qualified_name, signature, doc_comment, body_preview, ?1)
                     ORDER BY fts_score(symbol_name, qualified_name, signature, doc_comment, body_preview, ?1) DESC
                     LIMIT ?2",
                    (fts_q, k as i64)
                ).await?;
                let mut ranks = HashMap::new();
                let mut i = 0;
                while let Some(row) = rows.next().await? {
                    if let Ok(id) = row.get::<String>(0) {
                        ranks.insert(id, i);
                        i += 1;
                    }
                }
                ranks
            }
        };

        // Merge candidate IDs
        let mut all_ids: Vec<String> = vector_ranks.keys().cloned().collect();
        for id in fts_ranks.keys() {
            if !vector_ranks.contains_key(id) {
                all_ids.push(id.clone());
            }
        }

        if all_ids.is_empty() {
            return Ok(vec![]);
        }

        // Fetch metadata and compute RRF scores
        let placeholders = all_ids.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
        let sql = format!(
            "SELECT id, symbol_name, qualified_name, symbol_kind, file_path,
                    line_start, line_end, signature, doc_comment, churn_count, hotspot_score, language,
                    body_preview
             FROM index_chunks WHERE id IN ({placeholders})"
        );
        let mut rows = self
            .conn
            .query(&sql, turso::params_from_iter(all_ids.clone()))
            .await?;
        let mut scored: Vec<CodeResult> = Vec::new();
        while let Some(row) = rows.next().await? {
            let id: String = row.get(0)?;
            let rrf_v = search::rrf_score(vector_ranks.get(&id).copied());
            let rrf_f = search::rrf_score(fts_ranks.get(&id).copied());
            scored.push(CodeResult {
                id,
                symbol_name: row.get(1)?,
                qualified_name: row.get(2)?,
                symbol_kind: row.get(3)?,
                file_path: row.get(4)?,
                line_start: row.get(5)?,
                line_end: row.get(6)?,
                signature: row.get(7)?,
                doc_comment: row.get(8)?,
                churn_count: row.get(9).unwrap_or(0),
                hotspot_score: row.get(10).unwrap_or(0.0),
                language: row.get(11).unwrap_or_default(),
                body_preview: row.get(12).ok(),
                rrf_score: rrf_v + rrf_f,
                related_commits: vec![],
                duplicate_count: 0,
                owners: vec![],
            });
        }

        let mut scored = search::merge_and_dedup(scored, limit);

        let ids: Vec<String> = scored.iter().map(|r| r.id.clone()).collect();
        let commit_map = self.fetch_related_commits_batch(&ids, 3).await?;
        for result in &mut scored {
            result.related_commits = commit_map.get(&result.id).cloned().unwrap_or_default();
        }

        Ok(scored)
    }

    async fn get_symbol(
        &self,
        name_path: &str,
        file_path: Option<&str>,
    ) -> Result<Vec<CodeResult>> {
        let (sql, params): (String, Vec<String>) = if let Some(fp) = file_path {
            (
                "SELECT id, symbol_name, qualified_name, symbol_kind, file_path,
                        line_start, line_end, signature, doc_comment, churn_count, hotspot_score, language,
                        body_preview
                 FROM index_chunks
                 WHERE (symbol_name = ?1 OR qualified_name = ?1 OR qualified_name LIKE ?2)
                   AND file_path = ?3
                 ORDER BY line_start
                 LIMIT 20".to_string(),
                vec![name_path.to_string(), format!("%::{name_path}"), fp.to_string()],
            )
        } else {
            (
                "SELECT id, symbol_name, qualified_name, symbol_kind, file_path,
                        line_start, line_end, signature, doc_comment, churn_count, hotspot_score, language,
                        body_preview
                 FROM index_chunks
                 WHERE symbol_name = ?1 OR qualified_name = ?1 OR qualified_name LIKE ?2
                 ORDER BY hotspot_score DESC, churn_count DESC, line_start
                 LIMIT 20".to_string(),
                vec![name_path.to_string(), format!("%::{name_path}")],
            )
        };

        let mut rows = self.conn.query(&sql, turso::params_from_iter(params)).await?;
        let mut results = Vec::new();
        while let Some(row) = rows.next().await? {
            results.push(CodeResult {
                id: row.get(0)?,
                symbol_name: row.get(1)?,
                qualified_name: row.get(2)?,
                symbol_kind: row.get(3)?,
                file_path: row.get(4)?,
                line_start: row.get(5)?,
                line_end: row.get(6)?,
                signature: row.get(7)?,
                doc_comment: row.get(8)?,
                churn_count: row.get(9).unwrap_or(0),
                hotspot_score: row.get(10).unwrap_or(0.0),
                language: row.get(11).unwrap_or_default(),
                body_preview: row.get(12).ok(),
                rrf_score: 0.0,
                related_commits: vec![],
                duplicate_count: 0,
                owners: vec![],
            });
        }

        let ids: Vec<String> = results.iter().map(|r| r.id.clone()).collect();
        let commit_map = self.fetch_related_commits_batch(&ids, 5).await?;
        for r in &mut results {
            r.related_commits = commit_map.get(&r.id).cloned().unwrap_or_default();
        }

        // Ownership rollup per unique file
        {
            let mut seen = std::collections::HashSet::new();
            let mut file_owners: std::collections::HashMap<String, Vec<String>> =
                std::collections::HashMap::new();
            for r in &results {
                if seen.insert(r.file_path.clone()) {
                    let owners = self.get_file_owners(&r.file_path, 3).await?;
                    file_owners.insert(
                        r.file_path.clone(),
                        owners
                            .into_iter()
                            .map(|(author, cnt, ts)| format!("{author} ({cnt} commits, last {ts})"))
                            .collect(),
                    );
                }
            }
            for r in &mut results {
                r.owners = file_owners.get(&r.file_path).cloned().unwrap_or_default();
            }
        }
        Ok(results)
    }

    async fn get_coupled_files(
        &self,
        file_path: &str,
        limit: usize,
    ) -> Result<Vec<(String, i64, String)>> {
        let mut rows = self.conn.query(
            "SELECT
               CASE WHEN file_a = ?1 THEN file_b ELSE file_a END,
               shared_commits,
               last_shared_ts
             FROM index_file_coupling
             WHERE (file_a = ?1 OR file_b = ?1) AND shared_commits >= 2
             ORDER BY shared_commits DESC
             LIMIT ?2",
            (file_path.to_string(), limit as i64),
        ).await?;
        let mut results = Vec::new();
        while let Some(row) = rows.next().await? {
            results.push((
                row.get(0)?,
                row.get(1)?,
                row.get(2).unwrap_or_default(),
            ));
        }
        Ok(results)
    }

    async fn record_commit(
        &self,
        commit: &CommitInfo,
        file_updates: &[(String, i64, f64)],
    ) -> Result<()> {
        let conn = self.new_write_conn()?;

        conn.execute(
            "INSERT OR IGNORE INTO index_commits (sha, message) VALUES (?1, ?2)",
            (commit.sha.clone(), commit.message.clone()),
        )
        .await?;

        for (rel_path, churn_count, hotspot_score) in file_updates {
            conn.execute(
                "UPDATE index_chunks SET churn_count = ?1, hotspot_score = ?2 WHERE file_path = ?3",
                (*churn_count, *hotspot_score, rel_path.clone()),
            )
            .await?;

            let mut rows = conn
                .query(
                    "SELECT id FROM index_chunks WHERE file_path = ?1",
                    (rel_path.clone(),),
                )
                .await?;
            while let Some(row) = rows.next().await? {
                if let Ok(chunk_id) = row.get::<String>(0) {
                    let _ = conn.execute(
                        "INSERT OR IGNORE INTO index_chunk_commits (chunk_id, commit_sha) VALUES (?1, ?2)",
                        (chunk_id, commit.sha.clone()),
                    ).await;
                }
            }
        }

        Ok(())
    }
}

impl TursoStore {
    async fn fetch_related_commits_batch(
        &self,
        chunk_ids: &[String],
        per_chunk_limit: usize,
    ) -> Result<HashMap<String, Vec<String>>> {
        if chunk_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let placeholders = chunk_ids.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
        let sql = format!(
            "SELECT cc.chunk_id, c.message
             FROM index_commits c
             JOIN index_chunk_commits cc ON cc.commit_sha = c.sha
             WHERE cc.chunk_id IN ({placeholders})
             ORDER BY cc.chunk_id, c.sha"
        );
        let mut rows = self
            .conn
            .query(&sql, turso::params_from_iter(chunk_ids.iter().cloned()))
            .await?;
        let mut result: HashMap<String, Vec<String>> = HashMap::new();
        while let Some(row) = rows.next().await? {
            let chunk_id: String = row.get(0)?;
            let message: String = row.get(1)?;
            let msgs = result.entry(chunk_id).or_default();
            if msgs.len() < per_chunk_limit {
                msgs.push(message);
            }
        }
        Ok(result)
    }

    async fn get_file_owners(
        &self,
        file_path: &str,
        limit: usize,
    ) -> Result<Vec<(String, i64, String)>> {
        let mut rows = self.conn.query(
            "SELECT c.author, COUNT(*) as cnt, MAX(c.timestamp) as last_ts
             FROM index_commits c
             JOIN index_chunk_commits cc ON cc.commit_sha = c.sha
             JOIN index_chunks ic ON ic.id = cc.chunk_id
             WHERE ic.file_path = ?1
               AND c.author IS NOT NULL
               AND c.author != ''
             GROUP BY c.author
             ORDER BY cnt DESC, last_ts DESC
             LIMIT ?2",
            (file_path.to_string(), limit as i64),
        ).await?;
        let mut results = Vec::new();
        while let Some(row) = rows.next().await? {
            results.push((
                row.get(0)?,
                row.get(1)?,
                row.get(2).unwrap_or_default(),
            ));
        }
        Ok(results)
    }
}

pub(crate) fn build_fts_query(query: &str) -> String {
    query
        .split_whitespace()
        .filter_map(|w| {
            let clean: String = w
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if clean.is_empty() {
                None
            } else {
                Some(format!("{clean}*"))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::store::test_suite;

    async fn make_test_store() -> TursoStore {
        let db = turso::Builder::new_local(":memory:")
            .experimental_index_method(true)
            .build()
            .await
            .unwrap();
        let conn = Arc::new(db.connect().unwrap());
        TursoStore::ensure_schema(&conn).await.unwrap();
        TursoStore::new(db, conn, true)
    }

    #[tokio::test]
    async fn test_schema_idempotency() -> Result<()> {
        let db = turso::Builder::new_local(":memory:")
            .experimental_index_method(true)
            .build()
            .await?;
        let conn = Arc::new(db.connect()?);

        TursoStore::ensure_schema(&conn).await.context("First schema run failed")?;
        TursoStore::ensure_schema(&conn)
            .await
            .context("Second schema run failed (idempotency issue)")?;

        Ok(())
    }

    #[tokio::test]
    async fn test_logic_version_roundtrip() -> Result<()> {
        test_suite::test_logic_version_roundtrip(Arc::new(make_test_store().await)).await
    }

    #[tokio::test]
    async fn test_replace_and_search() -> Result<()> {
        test_suite::test_replace_and_search(Arc::new(make_test_store().await)).await
    }

    #[tokio::test]
    async fn test_get_symbol() -> Result<()> {
        test_suite::test_get_symbol(Arc::new(make_test_store().await)).await
    }

    #[tokio::test]
    async fn test_clear_all() -> Result<()> {
        test_suite::test_clear_all(Arc::new(make_test_store().await)).await
    }

    #[tokio::test]
    async fn test_remove_file() -> Result<()> {
        test_suite::test_remove_file(Arc::new(make_test_store().await)).await
    }

    #[tokio::test]
    async fn test_record_commit() -> Result<()> {
        test_suite::test_record_commit(Arc::new(make_test_store().await)).await
    }

    #[tokio::test]
    async fn test_zero_chunk_replace() -> Result<()> {
        test_suite::test_zero_chunk_replace_clears_stale_data(Arc::new(make_test_store().await)).await
    }

    #[test]
    fn fts_basic_terms() {
        let q = build_fts_query("index file");
        assert!(q.contains("index*"));
        assert!(q.contains("file*"));
    }

    #[test]
    fn fts_strips_special_chars() {
        let q = build_fts_query("file.path foo-bar");
        assert!(q.contains("filepath*"));
        assert!(q.contains("foobar*"));
    }

    #[test]
    fn fts_empty_tokens_dropped() {
        let q = build_fts_query("... --- !!!");
        assert!(q.is_empty());
    }

    #[test]
    fn fts_single_term() {
        let q = build_fts_query("ownership");
        assert_eq!(q, "ownership*");
    }

    #[test]
    fn fts_underscores_preserved() {
        let q = build_fts_query("my_function");
        assert!(q.contains("my_function*"));
    }

    #[test]
    fn fts_empty_input() {
        assert!(build_fts_query("").is_empty());
        assert!(build_fts_query("   ").is_empty());
    }
}
