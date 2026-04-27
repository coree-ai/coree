pub mod git;
pub mod indexer;
pub mod parser;
pub mod schema;
pub mod search;
pub mod watcher;

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::embed::Embedder;

pub struct IndexReady {
    /// Read connection for search queries. Never shared with write tasks.
    pub conn: Arc<turso::Connection>,
    pub embedder: Arc<Mutex<Embedder>>,
    pub project_root: PathBuf,
    pub git_history: bool,
    /// Kept alive so new_conn() can create independent write connections.
    db: turso::Database,
}

impl IndexReady {
    /// Create a fresh, independent connection to the index database.
    ///
    /// Each turso Connection has its own ConcurrentGuard (AtomicU32). Sharing
    /// a connection between concurrent tasks causes "concurrent use forbidden".
    /// Callers that write (indexer, watcher) must call this instead of cloning
    /// `conn`, so they never contend with search query handlers.
    pub fn new_conn(&self) -> Result<Arc<turso::Connection>> {
        let conn = self
            .db
            .connect()
            .context("Failed to create index connection")?;
        Ok(Arc::new(conn))
    }
}

#[derive(Clone)]
pub enum IndexState {
    /// Index DB is being opened and schema applied.
    Opening,
    /// Index is open and ready for queries; indexing may be in progress.
    Ready(Arc<IndexReady>),
    /// Indexing is disabled in config.
    Disabled,
    /// Init failed permanently.
    Failed(String),
}

const INDEX_OPEN_ATTEMPTS: u32 = 20;
const INDEX_OPEN_RETRY_MS: u64 = 250;

/// Open the index database at `db_path`, apply schema, return an `IndexReady`.
///
/// Retries up to INDEX_OPEN_ATTEMPTS times to handle the process-handover race:
/// the previous serve process may still hold the file lock briefly after
/// serve.lock is released. Mirrors the replica retry pattern in db.rs.
pub async fn open(
    db_path: &std::path::Path,
    project_root: PathBuf,
    git_history: bool,
    embedder: Arc<Mutex<Embedder>>,
) -> Result<IndexReady> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let db_path_str = db_path
        .to_str()
        .context("Index DB path is not valid UTF-8")?
        .to_string();

    let db = {
        let mut last_err = None;
        let mut db = None;
        for attempt in 1..=INDEX_OPEN_ATTEMPTS {
            match turso::Builder::new_local(&db_path_str)
                .experimental_multiprocess_wal(true)
                .experimental_index_method(true)
                .build()
                .await
            {
                Ok(d) => {
                    db = Some(d);
                    break;
                }
                Err(e) => {
                    tracing::debug!(attempt, error = %e, "index DB open failed, retrying...");
                    last_err = Some(e);
                    tokio::time::sleep(std::time::Duration::from_millis(INDEX_OPEN_RETRY_MS)).await;
                    let _ = attempt;
                }
            }
        }
        db.ok_or_else(|| {
            anyhow::anyhow!(
                "Failed to open index DB after {INDEX_OPEN_ATTEMPTS} attempts: {}",
                last_err.map(|e| e.to_string()).unwrap_or_default()
            )
        })?
    };

    let conn = Arc::new(
        db.connect()
            .context("Failed to connect to index database")?,
    );
    schema::ensure(&conn).await?;

    Ok(IndexReady {
        conn,
        embedder,
        project_root,
        git_history,
        db,
    })
}
