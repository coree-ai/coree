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
    pub conn: Arc<turso::Connection>,
    pub embedder: Arc<Mutex<Embedder>>,
    pub project_root: PathBuf,
    pub git_history: bool,
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

/// Open the index database at `db_path`, apply schema, return an `IndexReady`.
pub async fn open(
    db_path: &std::path::Path,
    project_root: PathBuf,
    git_history: bool,
    embedder: Arc<Mutex<Embedder>>,
) -> Result<IndexReady> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    
    let db_path_str = db_path.to_str().context("Index DB path is not valid UTF-8")?.to_string();
    let db = turso::Builder::new_local(&db_path_str)
        .experimental_multiprocess_wal(true)
        .experimental_index_method(true)
        .build()
        .await
        .with_context(|| format!("Failed to open local index DB at {}", db_path.display()))?;

    let conn = db.connect().context("Failed to connect to index database")?;
    let conn = Arc::new(conn);
    schema::ensure(&conn).await?;
    
    Ok(IndexReady { conn, embedder, project_root, git_history })
}
