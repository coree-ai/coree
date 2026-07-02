pub mod git;
pub mod indexer;
pub mod parser;
pub mod search;
pub mod sqlite_store;
pub mod store;
pub mod turso_store;
pub mod watcher;

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::config::IndexBackend;
use crate::embed::Embedder;
use store::CodeIndexStore;

pub const INDEX_LOGIC_VERSION: u32 = 2;

pub struct IndexReady {
    pub store: Arc<dyn CodeIndexStore>,
    pub embedder: Arc<Mutex<Embedder>>,
    pub project_root: PathBuf,
    pub git_history: bool,
}

#[derive(Clone)]
pub enum IndexState {
    Opening,
    Ready(Arc<IndexReady>),
    Disabled,
    Failed(String),
}

const INDEX_OPEN_ATTEMPTS: u32 = 20;
const INDEX_OPEN_RETRY_MS: u64 = 250;

pub async fn open(
    db_path: &std::path::Path,
    backend: &IndexBackend,
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

    let store: Arc<dyn CodeIndexStore> = match backend {
        IndexBackend::Turso => {
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
                        }
                    }
                }
                db.ok_or_else(|| {
                    anyhow::anyhow!(
                        "Failed to open turso index DB after {INDEX_OPEN_ATTEMPTS} attempts: {}",
                        last_err.map(|e| e.to_string()).unwrap_or_default()
                    )
                })?
            };

            let conn = Arc::new(
                db.connect()
                    .context("Failed to connect to index database")?,
            );
            turso_store::TursoStore::ensure_schema(&conn).await?;

            Arc::new(turso_store::TursoStore::new(db, conn, git_history))
        }
        IndexBackend::Sqlite => {
            Arc::new(sqlite_store::SqliteStore::open(&db_path_str, git_history)?)
        }
    };

    Ok(IndexReady {
        store,
        embedder,
        project_root,
        git_history,
    })
}

pub async fn needs_rebuild(store: &Arc<dyn CodeIndexStore>) -> Result<bool> {
    let stored = store.stored_logic_version().await?;
    Ok(stored != Some(INDEX_LOGIC_VERSION))
}

pub async fn reset_stored_version(db_path: &std::path::Path, backend: &IndexBackend) -> Result<()> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let db_path_str = db_path
        .to_str()
        .context("Index DB path is not valid UTF-8")?
        .to_string();

    match backend {
        IndexBackend::Turso => {
            let db = turso::Builder::new_local(&db_path_str)
                .experimental_multiprocess_wal(true)
                .experimental_index_method(true)
                .build()
                .await
                .context("Failed to open index DB for version reset")?;
            let conn = Arc::new(db.connect().context("Failed to connect to index DB")?);
            turso_store::TursoStore::ensure_schema(&conn).await?;
            conn.execute(
                "INSERT OR REPLACE INTO meta (key, value) VALUES ('index_logic_version', '0')",
                (),
            )
            .await?;
        }
        IndexBackend::Sqlite => {
            let conn = rusqlite::Connection::open(db_path_str)
                .context("Failed to open sqlite index DB for version reset")?;
            conn.pragma_update(None, "journal_mode", "WAL")?;
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                 INSERT OR REPLACE INTO meta (key, value) VALUES ('index_logic_version', '0');",
            )?;
        }
    }

    Ok(())
}
