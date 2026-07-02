use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use uuid::Uuid;

use super::git::CommitInfo;
use super::search::{self, CodeResult};
use super::store::{CodeIndexStore, IndexedFile};
use crate::embed;

fn ensure_vec_extension() {
    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(|| {
        unsafe {
            rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute::<
                *const (),
                unsafe extern "C" fn(
                    *mut rusqlite::ffi::sqlite3,
                    *mut *mut std::os::raw::c_char,
                    *const rusqlite::ffi::sqlite3_api_routines,
                ) -> i32,
            >(
                sqlite_vec::sqlite3_vec_init as *const (),
            )));
        }
    });
}

pub struct SqliteStore {
    conn: Arc<Mutex<rusqlite::Connection>>,
    git_history: bool,
}

impl SqliteStore {
    pub fn open(path: &str, git_history: bool) -> Result<Self> {
        // Registration must happen before the connection is created:
        // sqlite3_auto_extension only applies to subsequently opened connections.
        ensure_vec_extension();
        let conn = rusqlite::Connection::open(path)
            .context("Failed to open sqlite index database")?;
        Self::new(conn, git_history)
    }

    /// `conn` must have been opened after vec0 registration (use `open` or
    /// `new_in_memory` unless you have called `ensure_vec_extension` yourself).
    pub fn new(conn: rusqlite::Connection, git_history: bool) -> Result<Self> {
        ensure_vec_extension();
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
            git_history,
        };
        store.ensure_schema()?;
        Ok(store)
    }

    pub fn new_in_memory() -> Result<Self> {
        ensure_vec_extension();
        let conn = rusqlite::Connection::open_in_memory()?;
        Self::new(conn, true)
    }

    fn ensure_schema(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "busy_timeout", "5000")?;

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
             CREATE INDEX IF NOT EXISTS index_chunks_file ON index_chunks (file_path);
             CREATE VIRTUAL TABLE IF NOT EXISTS index_vectors USING vec0(
                 chunk_id    TEXT PRIMARY KEY,
                 embedding   float[384] distance_metric=cosine,
                 embed_model TEXT
             );
             CREATE VIRTUAL TABLE IF NOT EXISTS index_chunks_fts USING fts5(
                 symbol_name, qualified_name, signature, doc_comment, body_preview,
                 chunk_id UNINDEXED
             );
             CREATE TABLE IF NOT EXISTS index_commits (
                 sha       TEXT PRIMARY KEY,
                 message   TEXT NOT NULL,
                 author    TEXT,
                 timestamp TEXT
             );
             CREATE TABLE IF NOT EXISTS index_chunk_commits (
                 chunk_id   TEXT NOT NULL,
                 commit_sha TEXT NOT NULL,
                 PRIMARY KEY (chunk_id, commit_sha)
             );
             CREATE INDEX IF NOT EXISTS index_chunk_commits_by_sha
                 ON index_chunk_commits (commit_sha);
             CREATE TABLE IF NOT EXISTS index_file_coupling (
                 file_a         TEXT NOT NULL,
                 file_b         TEXT NOT NULL,
                 shared_commits INTEGER NOT NULL DEFAULT 0,
                 last_shared_ts TEXT NOT NULL,
                 PRIMARY KEY (file_a, file_b)
             );
             CREATE TABLE IF NOT EXISTS meta (
                 key   TEXT PRIMARY KEY,
                 value TEXT NOT NULL
             );",
        )?;

        Ok(())
    }
}

#[async_trait]
impl CodeIndexStore for SqliteStore {
    async fn stored_logic_version(&self) -> Result<Option<u32>> {
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let mut stmt = conn
                .prepare("SELECT value FROM meta WHERE key = 'index_logic_version'")?;
            let mut rows = stmt.query(())?;
            if let Some(row) = rows.next()? {
                let v: String = row.get(0)?;
                v.parse::<u32>()
                    .ok()
                    .map(Some)
                    .ok_or_else(|| anyhow::anyhow!("Invalid index_logic_version in meta: {v}"))
            } else {
                Ok(None)
            }
        })
        .await?
    }

    async fn set_stored_logic_version(&self, version: u32) -> Result<()> {
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            conn.execute(
                "INSERT OR REPLACE INTO meta (key, value) VALUES ('index_logic_version', ?1)",
                rusqlite::params![version.to_string()],
            )?;
            Ok(())
        })
        .await?
    }

    async fn clear_all(&self) -> Result<()> {
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            conn.execute("DELETE FROM index_vectors", ())?;
            conn.execute("DELETE FROM index_chunks_fts", ())?;
            conn.execute("DELETE FROM index_chunk_commits", ())?;
            conn.execute("DELETE FROM index_chunks", ())?;
            conn.execute("DELETE FROM index_files", ())?;
            conn.execute("DELETE FROM index_commits", ())?;
            conn.execute("DELETE FROM index_file_coupling", ())?;
            Ok(())
        })
        .await?
    }

    async fn file_content_hash(&self, rel_path: &str) -> Result<Option<String>> {
        let path = rel_path.to_string();
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let mut stmt = conn
                .prepare("SELECT content_hash FROM index_files WHERE path = ?1")?;
            let mut rows = stmt.query(rusqlite::params![path])?;
            Ok(rows.next()?.map(|r| r.get(0)).transpose()?)
        })
        .await?
    }

    async fn replace_file(&self, file: IndexedFile) -> Result<()> {
        let conn = self.conn.clone();
        let git_history = self.git_history;
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let tx = conn.unchecked_transaction()?;
            let rel_path = file.rel_path.clone();
            let now = Utc::now().to_rfc3339();
            let model_id = crate::embed::model_id();

            tx.execute(
                "DELETE FROM index_vectors WHERE chunk_id IN (SELECT id FROM index_chunks WHERE file_path = ?1)",
                rusqlite::params![rel_path],
            )?;
            tx.execute(
                "DELETE FROM index_chunk_commits WHERE chunk_id IN (SELECT id FROM index_chunks WHERE file_path = ?1)",
                rusqlite::params![rel_path],
            )?;
            tx.execute(
                "DELETE FROM index_chunks_fts WHERE chunk_id IN (SELECT id FROM index_chunks WHERE file_path = ?1)",
                rusqlite::params![rel_path],
            )?;

            tx.execute(
                "DELETE FROM index_chunks WHERE file_path = ?1",
                rusqlite::params![rel_path],
            )?;

            // Upsert commits
            for commit in &file.commits {
                let timestamp = chrono::DateTime::from_timestamp(commit.timestamp_unix, 0)
                    .map(|dt| dt.to_rfc3339())
                    .unwrap_or_default();
                tx.execute(
                    "INSERT OR IGNORE INTO index_commits (sha, message, author, timestamp) \
                     VALUES (?1, ?2, ?3, ?4)",
                    rusqlite::params![
                        commit.sha,
                        commit.message,
                        commit.author,
                        timestamp,
                    ],
                )?;
            }

            // Insert chunks, vectors, FTS, and chunk-commits
            for ic in &file.chunks {
                let blob = embed::floats_to_blob(&ic.embedding);
                let chunk_id = Uuid::new_v4().to_string();

                tx.execute(
                    "INSERT INTO index_chunks \
                     (id, file_path, symbol_name, qualified_name, symbol_kind, signature, \
                      doc_comment, body_preview, line_start, line_end, language, \
                      churn_count, hotspot_score, indexed_at, content_hash) \
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
                    rusqlite::params![
                        chunk_id,
                        rel_path,
                        ic.chunk.symbol_name,
                        ic.chunk.qualified_name,
                        ic.chunk.symbol_kind,
                        ic.chunk.signature,
                        ic.chunk.doc_comment,
                        ic.chunk.body_preview,
                        ic.chunk.line_start as i64,
                        ic.chunk.line_end as i64,
                        file.language,
                        file.churn_count,
                        file.hotspot_score,
                        now,
                        ic.content_hash,
                    ],
                )?;

                tx.execute(
                    "INSERT INTO index_vectors (chunk_id, embedding, embed_model) \
                     VALUES (?1, ?2, ?3)",
                    rusqlite::params![chunk_id, blob, model_id],
                )?;

                tx.execute(
                    "INSERT INTO index_chunks_fts \
                     (symbol_name, qualified_name, signature, doc_comment, body_preview, chunk_id) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    rusqlite::params![
                        ic.chunk.symbol_name,
                        ic.chunk.qualified_name,
                        ic.chunk.signature,
                        ic.chunk.doc_comment,
                        ic.chunk.body_preview,
                        chunk_id,
                    ],
                )?;

                for commit in &file.commits {
                    tx.execute(
                        "INSERT OR IGNORE INTO index_chunk_commits (chunk_id, commit_sha) \
                         VALUES (?1, ?2)",
                        rusqlite::params![chunk_id, commit.sha],
                    )?;
                }
            }

            // Refresh file coupling
            if git_history {
                tx.execute(
                    "DELETE FROM index_file_coupling WHERE file_a = ?1 OR file_b = ?1",
                    rusqlite::params![rel_path],
                )?;

                tx.execute(
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
                    rusqlite::params![rel_path],
                )?;
            }

            // Upsert file hash
            let now = Utc::now().to_rfc3339();
            tx.execute(
                "INSERT OR REPLACE INTO index_files (path, content_hash, indexed_at) \
                 VALUES (?1, ?2, ?3)",
                rusqlite::params![rel_path, file.content_hash, now],
            )?;

            tx.commit()?;
            Ok(())
        })
        .await?
    }

    async fn remove_file(&self, rel_path: &str) -> Result<()> {
        let path = rel_path.to_string();
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let tx = conn.unchecked_transaction()?;

            tx.execute(
                "DELETE FROM index_vectors WHERE chunk_id IN (SELECT id FROM index_chunks WHERE file_path = ?1)",
                rusqlite::params![path],
            )?;
            tx.execute(
                "DELETE FROM index_chunk_commits WHERE chunk_id IN (SELECT id FROM index_chunks WHERE file_path = ?1)",
                rusqlite::params![path],
            )?;
            tx.execute(
                "DELETE FROM index_chunks_fts WHERE chunk_id IN (SELECT id FROM index_chunks WHERE file_path = ?1)",
                rusqlite::params![path],
            )?;

            tx.execute("DELETE FROM index_chunks WHERE file_path = ?1", rusqlite::params![path])?;
            tx.execute("DELETE FROM index_files WHERE path = ?1", rusqlite::params![path])?;

            tx.commit()?;
            Ok(())
        })
        .await?
    }

    async fn search_code(
        &self,
        embedding: Vec<f32>,
        query: &str,
        limit: usize,
    ) -> Result<Vec<CodeResult>> {
        let blob = embed::floats_to_blob(&embedding);
        let model = embed::model_id();
        let k = (limit * 2) as i64;
        let fts_query = build_fts5_query(query);
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || -> Result<Vec<CodeResult>> {
            let conn = conn.lock().unwrap();

            // Stream A: vector search via vec0 KNN
            let mut vector_ranks: HashMap<String, usize> = HashMap::new();
            {
                let mut stmt = conn.prepare(
                    "SELECT chunk_id, distance
                     FROM index_vectors
                     WHERE embedding MATCH ?1 AND k = ?2 AND embed_model = ?3",
                )?;
                let mut rank = 0usize;
                let mut rows = stmt.query(rusqlite::params![blob, k, model])?;
                while let Some(row) = rows.next()? {
                    let id: String = row.get(0)?;
                    vector_ranks.insert(id, rank);
                    rank += 1;
                }
            }

            // Stream B: FTS5 keyword search
            let fts_ranks: HashMap<String, usize> = if fts_query.is_empty() {
                HashMap::new()
            } else {
                let mut stmt = conn.prepare(
                    "SELECT chunk_id
                     FROM index_chunks_fts
                     WHERE index_chunks_fts MATCH ?1
                     ORDER BY rank
                     LIMIT ?2",
                )?;
                let mut ranks = HashMap::new();
                let mut i = 0;
                let mut rows = stmt.query(rusqlite::params![fts_query, k])?;
                while let Some(row) = rows.next()? {
                    let id: String = row.get(0)?;
                    ranks.insert(id, i);
                    i += 1;
                }
                ranks
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

            let params: Vec<Box<dyn rusqlite::types::ToSql>> = all_ids
                .iter()
                .map(|s| Box::new(s.clone()) as Box<dyn rusqlite::types::ToSql>)
                .collect();
            let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();

            let mut stmt = conn.prepare(&sql)?;
            let mut rows = stmt.query(param_refs.as_slice())?;

            let mut scored: Vec<CodeResult> = Vec::new();
            while let Some(row) = rows.next()? {
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
            let commit_map = fetch_related_commits_batch(&conn, &ids, 3)?;
            for result in &mut scored {
                result.related_commits = commit_map.get(&result.id).cloned().unwrap_or_default();
            }

            Ok(scored)
        })
        .await?
    }

    async fn get_symbol(
        &self,
        name_path: &str,
        file_path: Option<&str>,
    ) -> Result<Vec<CodeResult>> {
        let name = name_path.to_string();
        let like_pattern = format!("%::{name_path}");
        let fp = file_path.map(|s| s.to_string());
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || -> Result<Vec<CodeResult>> {
            let conn = conn.lock().unwrap();
            let mut results = Vec::new();

            if let Some(ref fp) = fp {
                let mut stmt = conn.prepare(
                    "SELECT id, symbol_name, qualified_name, symbol_kind, file_path,
                            line_start, line_end, signature, doc_comment, churn_count, hotspot_score, language,
                            body_preview
                     FROM index_chunks
                     WHERE (symbol_name = ?1 OR qualified_name = ?1 OR qualified_name LIKE ?2)
                       AND file_path = ?3
                     ORDER BY line_start
                     LIMIT 20",
                )?;
                let mut rows = stmt.query(rusqlite::params![name, like_pattern, fp])?;
                while let Some(row) = rows.next()? {
                    results.push(read_code_result(row)?);
                }
            } else {
                let mut stmt = conn.prepare(
                    "SELECT id, symbol_name, qualified_name, symbol_kind, file_path,
                            line_start, line_end, signature, doc_comment, churn_count, hotspot_score, language,
                            body_preview
                     FROM index_chunks
                     WHERE symbol_name = ?1 OR qualified_name = ?1 OR qualified_name LIKE ?2
                     ORDER BY hotspot_score DESC, churn_count DESC, line_start
                     LIMIT 20",
                )?;
                let mut rows = stmt.query(rusqlite::params![name, like_pattern])?;
                while let Some(row) = rows.next()? {
                    results.push(read_code_result(row)?);
                }
            }

            let ids: Vec<String> = results.iter().map(|r| r.id.clone()).collect();
            let commit_map = fetch_related_commits_batch(&conn, &ids, 5)?;
            for r in &mut results {
                r.related_commits = commit_map.get(&r.id).cloned().unwrap_or_default();
            }

            // Ownership rollup per unique file
            {
                let mut seen = std::collections::HashSet::new();
                let mut file_owners: HashMap<String, Vec<String>> = HashMap::new();
                for r in &results {
                    if seen.insert(r.file_path.clone()) {
                        let owners = get_file_owners(&conn, &r.file_path, 3)?;
                        file_owners.insert(
                            r.file_path.clone(),
                            owners
                                .into_iter()
                                .map(|(author, cnt, ts)| {
                                    format!("{author} ({cnt} commits, last {ts})")
                                })
                                .collect(),
                        );
                    }
                }
                for r in &mut results {
                    r.owners = file_owners.get(&r.file_path).cloned().unwrap_or_default();
                }
            }

            Ok(results)
        })
        .await?
    }

    async fn get_coupled_files(
        &self,
        file_path: &str,
        limit: usize,
    ) -> Result<Vec<(String, i64, String)>> {
        let path = file_path.to_string();
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let mut stmt = conn.prepare(
                "SELECT
                   CASE WHEN file_a = ?1 THEN file_b ELSE file_a END,
                   shared_commits,
                   last_shared_ts
                 FROM index_file_coupling
                 WHERE (file_a = ?1 OR file_b = ?1) AND shared_commits >= 2
                 ORDER BY shared_commits DESC
                 LIMIT ?2",
            )?;
            let mut rows = stmt.query(rusqlite::params![path, limit as i64])?;
            let mut results = Vec::new();
            while let Some(row) = rows.next()? {
                results.push((row.get(0)?, row.get(1)?, row.get(2).unwrap_or_default()));
            }
            Ok(results)
        })
        .await?
    }

    async fn record_commit(
        &self,
        commit: &CommitInfo,
        file_updates: &[(String, i64, f64)],
    ) -> Result<()> {
        let sha = commit.sha.clone();
        let message = commit.message.clone();
        let updates = file_updates.to_vec();
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            conn.execute(
                "INSERT OR IGNORE INTO index_commits (sha, message) VALUES (?1, ?2)",
                rusqlite::params![sha, message],
            )?;
            for (rel_path, churn_count, hotspot_score) in &updates {
                conn.execute(
                    "UPDATE index_chunks SET churn_count = ?1, hotspot_score = ?2 WHERE file_path = ?3",
                    rusqlite::params![churn_count, hotspot_score, rel_path],
                )?;
                let mut stmt = conn.prepare(
                    "SELECT id FROM index_chunks WHERE file_path = ?1",
                )?;
                let mut rows = stmt.query(rusqlite::params![rel_path])?;
                while let Some(row) = rows.next()? {
                    let chunk_id: String = row.get(0)?;
                    let _ = conn.execute(
                        "INSERT OR IGNORE INTO index_chunk_commits (chunk_id, commit_sha) VALUES (?1, ?2)",
                        rusqlite::params![chunk_id, sha],
                    );
                }
            }
            Ok(())
        })
        .await?
    }
}

fn read_code_result(row: &rusqlite::Row) -> Result<CodeResult> {
    Ok(CodeResult {
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
    })
}

fn fetch_related_commits_batch(
    conn: &rusqlite::Connection,
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
    let params: Vec<Box<dyn rusqlite::types::ToSql>> = chunk_ids
        .iter()
        .map(|s| Box::new(s.clone()) as Box<dyn rusqlite::types::ToSql>)
        .collect();
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();

    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(param_refs.as_slice())?;
    let mut result: HashMap<String, Vec<String>> = HashMap::new();
    while let Some(row) = rows.next()? {
        let chunk_id: String = row.get(0)?;
        let message: String = row.get(1)?;
        let msgs = result.entry(chunk_id).or_default();
        if msgs.len() < per_chunk_limit {
            msgs.push(message);
        }
    }
    Ok(result)
}

fn get_file_owners(
    conn: &rusqlite::Connection,
    file_path: &str,
    limit: usize,
) -> Result<Vec<(String, i64, String)>> {
    let mut stmt = conn.prepare(
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
    )?;
    let mut rows = stmt.query(rusqlite::params![file_path, limit as i64])?;
    let mut results = Vec::new();
    while let Some(row) = rows.next()? {
        results.push((row.get(0)?, row.get(1)?, row.get(2).unwrap_or_default()));
    }
    Ok(results)
}

fn build_fts5_query(query: &str) -> String {
    let terms: Vec<String> = query
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
        .collect();
    if terms.is_empty() {
        String::new()
    } else {
        terms.join(" OR ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::store::test_suite;

    fn make_test_store() -> SqliteStore {
        SqliteStore::new_in_memory().unwrap()
    }

    #[tokio::test]
    async fn test_schema_idempotency() -> Result<()> {
        let store = SqliteStore::new_in_memory()?;
        store.ensure_schema()?;
        Ok(())
    }

    #[tokio::test]
    async fn test_logic_version_roundtrip() -> Result<()> {
        test_suite::test_logic_version_roundtrip(Arc::new(make_test_store())).await
    }

    #[tokio::test]
    async fn test_replace_and_search() -> Result<()> {
        test_suite::test_replace_and_search(Arc::new(make_test_store())).await
    }

    #[tokio::test]
    async fn test_get_symbol() -> Result<()> {
        test_suite::test_get_symbol(Arc::new(make_test_store())).await
    }

    #[tokio::test]
    async fn test_clear_all() -> Result<()> {
        test_suite::test_clear_all(Arc::new(make_test_store())).await
    }

    #[tokio::test]
    async fn test_remove_file() -> Result<()> {
        test_suite::test_remove_file(Arc::new(make_test_store())).await
    }

    #[tokio::test]
    async fn test_record_commit() -> Result<()> {
        test_suite::test_record_commit(Arc::new(make_test_store())).await
    }

    #[tokio::test]
    async fn test_zero_chunk_replace() -> Result<()> {
        test_suite::test_zero_chunk_replace_clears_stale_data(Arc::new(make_test_store())).await
    }

    #[test]
    fn fts5_basic_terms() {
        let q = build_fts5_query("index file");
        assert!(q.contains("index*"));
        assert!(q.contains("file*"));
        assert!(q.contains(" OR "));
    }

    #[test]
    fn fts5_strips_special_chars() {
        let q = build_fts5_query("file.path foo-bar");
        assert!(q.contains("filepath*"));
        assert!(q.contains("foobar*"));
    }

    #[test]
    fn fts5_empty_tokens_dropped() {
        let q = build_fts5_query("... --- !!!");
        assert!(q.is_empty());
    }

    #[test]
    fn fts5_single_term() {
        let q = build_fts5_query("ownership");
        assert_eq!(q, "ownership*");
    }

    #[test]
    fn fts5_empty_input() {
        assert!(build_fts5_query("").is_empty());
        assert!(build_fts5_query("   ").is_empty());
    }
}
