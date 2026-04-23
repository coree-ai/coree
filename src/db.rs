use anyhow::{Context, Result};
use libsql::{Builder, Connection, Database};
use std::path::Path;

use crate::{config::{StorageMode, RemoteMode, Config}, mlog};

pub struct Db {
    pub conn: Connection,
    _db: Database,
}

impl Db {
    pub async fn open(config: &Config) -> Result<Self> {
        let t = std::time::Instant::now();
        let s = &config.memory.storage;
        let db = match s.mode {
            StorageMode::Managed | StorageMode::Local | StorageMode::Disabled => {
                let path = config.db_path();
                ensure_parent_dir(&path)?;
                Builder::new_local(&path)
                    .build()
                    .await
                    .with_context(|| format!("Failed to open local DB at {}", path.display()))?
            }
            StorageMode::Remote => {
                let url = s
                    .remote_url
                    .as_deref()
                    .context("remote mode requires memory.remote_url")?;
                let token = s
                    .remote_auth_token
                    .as_deref()
                    .context("remote mode requires memory.remote_auth_token")?;
                match s.remote_mode {
                    RemoteMode::Direct => {
                        // No local file for direct mode; ensure the managed dir exists for
                        // serve.lock, serve.ready, and crash.log.
                        ensure_parent_dir(&config.db_path())?;
                        Builder::new_remote(url.to_string(), token.to_string())
                            .build()
                            .await
                            .with_context(|| format!("Failed to connect to remote at {url}"))?
                    }
                    RemoteMode::Replica => {
                        let path = config.db_path();
                        ensure_parent_dir(&path)?;
                        let path_str = path.to_str().context("replica DB path is not valid UTF-8")?;
                        // No timeout: initial sync duration scales with DB size and network speed.
                        // A hard timeout risks killing mid-sync and leaving the replica in a
                        // partial state. libsql WAL is crash-safe so a SIGTERM mid-sync is
                        // recoverable via the purge-and-retry path in open_replica_with_recovery.
                        open_replica_with_recovery(path_str, path.as_ref(), url, token).await?
                    }
                }
            }
        };

        let conn = db.connect().context("Failed to connect to database")?;

        // Enable WAL mode and a generous busy timeout for local mode only.
        // WAL allows concurrent readers while a writer holds the lock; busy_timeout
        // makes writers retry for up to 5s instead of immediately returning SQLITE_BUSY.
        // This makes local mode safe for multiple concurrent tyto processes (e.g.
        // multiple agents or IDE windows on the same project).
        //
        // Skipped for replica mode: the local replica file is managed by libsql's
        // sync engine and pragma behaviour there is undocumented - leave it alone.
        //
        // Known gap: the in-process WriteLock dedup guard does not extend across
        // processes, so concurrent agents may occasionally write duplicate memories.
        // Acceptable for v1; a shared-lock or daemon model can address this later.
        if matches!(s.mode, StorageMode::Managed | StorageMode::Local) {
            conn.execute_batch(
                "PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;"
            )
            .await
            .context("Failed to set WAL mode / busy_timeout")?;
        }

        tracing::debug!(elapsed_ms = t.elapsed().as_millis(), "Db::open");
        Ok(Self { conn, _db: db })
    }
}

/// Open a remote replica, automatically recovering from local file corruption.
///
/// If the initial open fails (e.g. "database disk image is malformed" after a
/// mid-write process kill), delete all local replica files and retry once.
/// The remote is the source of truth, so this is always safe.
///
/// # Known failure mode: replica stuck in bad state after auth failure
///
/// Observed sequence (2026-04-11):
///   1. MCP server starts without auth token -> `build()` fails with auth error.
///   2. Recovery calls `purge_replica_files`. If the replica file was never
///      created (build failed before writing it), `remove_file` gets ENOENT even
///      though `exists()` returned true a moment earlier (TOCTOU race - libsql's
///      own builder may clean up the partial file between the two calls).
///   3. That ENOENT propagates as a crash, written to crash.log.
///   4. Next session: replica file is gone. libsql must do a full initial sync
///      (~10s+), which exceeds the previous 10s hard timeout -> stuck in timeout
///      loop across sessions.
///
/// Fixes applied:
///   - `purge_replica_files` now uses attempt-and-ignore-NotFound instead of
///     exists()-then-remove (eliminates TOCTOU).
///   - Timeout is 60s when replica file is absent (initial/post-purge sync),
///     10s when it already exists (incremental sync only).
///
/// TODO: surface this state to the user more gracefully:
///   - Detect "replica missing after previous crash" and emit a clear message.
///   - Consider a `tyto remote reset` command that purges local replica files
///     and forces a clean re-sync, giving the user a self-service recovery path.
///   - Track whether the last open was a fresh sync vs incremental to give
///     better timeout/progress feedback.
async fn open_replica_with_recovery(
    path_str: &str,
    path: &Path,
    url: &str,
    token: &str,
) -> Result<Database> {
    let build = || {
        Builder::new_remote_replica(path_str, url.to_string(), token.to_string())
            .sync_interval(std::time::Duration::from_secs(1))
            .build()
    };

    // Try build + sync, treating any error as a corruption signal worth purging for.
    // Returns Ok(db) only when both build and sync succeed.
    let try_open = || async {
        let db = build().await?;
        let t = std::time::Instant::now();
        db.sync().await.map_err(|e| anyhow::anyhow!("replica sync failed: {e:#}"))?;
        tracing::debug!(elapsed_ms = t.elapsed().as_millis(), "replica sync");
        Ok::<_, anyhow::Error>(db)
    };

    // First attempt: use existing local files (fast incremental sync).
    match try_open().await {
        Ok(db) => return Ok(db),
        Err(e) => mlog!("tyto: replica open failed ({e:#}), purging and retrying..."),
    }

    // Recovery: purge all replica files and force a full re-sync from remote.
    // Replica files are distinct from local-mode `memory.db`, so this is always safe.
    purge_replica_files(path)?;

    // Second attempt: fresh sync from Turso. If this also fails, surface the error.
    try_open().await.with_context(|| {
        format!("Failed to open replica DB at {} (after recovery attempt)", path.display())
    })
}

/// Delete all libsql replica local files so the next open does a clean re-sync.
/// Deletes every file in the same directory whose name starts with the replica
/// filename — this covers any suffix libsql uses (.db, -shm, -wal, -info, -meta,
/// future variants) without needing to enumerate them explicitly.
pub fn purge_replica_files(path: &Path) -> Result<()> {
    let parent = path.parent().unwrap_or(std::path::Path::new("."));
    let prefix = path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_string();

    let entries = std::fs::read_dir(parent)
        .with_context(|| format!("Failed to read dir {}", parent.display()))?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with(&prefix) {
            match std::fs::remove_file(entry.path()) {
                Ok(()) => tracing::debug!(file = %name_str, "purged replica file"),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e).with_context(|| format!("Failed to remove {}", entry.path().display())),
            }
        }
    }
    Ok(())
}

fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory {}", parent.display()))?;
    }
    Ok(())
}
