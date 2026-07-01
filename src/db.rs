use anyhow::{Context, Result};
use std::path::Path;
use turso::{Connection, Database};

use crate::{
    config::{Config, RemoteMode, StorageMode},
    mlog,
};

pub enum AnyDb {
    Local(Database),
    Synced(turso::sync::Database),
}

pub struct Db {
    pub conn: Connection,
    pub handle: AnyDb,
    // Keeps the temp directory alive for direct-mode replicas. None for all other modes.
    #[allow(dead_code)]
    pub temp_dir: Option<tempfile::TempDir>,
}

impl Db {
    /// Open the memory database. `can_purge` authorises destructive replica
    /// recovery (back up + re-pull) on a failed replica open: pass `true` only
    /// from the serve primary, which holds `serve.lock` and therefore owns the
    /// replica files. Read-only callers (e.g. `status`) pass `false` so they
    /// never disturb a replica out from under a running serve.
    pub async fn open(config: &Config, can_purge: bool) -> Result<Self> {
        let t = std::time::Instant::now();
        let s = &config.memory.storage;
        let (any_db, temp_dir) = match s.mode {
            StorageMode::Managed | StorageMode::Local | StorageMode::Disabled => {
                let path = config.db_path();
                ensure_parent_dir(&path)?;
                mlog!("coree: opening local database at {} (mode {:?})", path.display(), s.mode);
                let db =
                    turso::Builder::new_local(path.to_str().context("DB path is not valid UTF-8")?)
                        .experimental_index_method(true)
                        .build()
                        .await
                        .with_context(|| {
                            format!("Failed to open local DB at {}", path.display())
                        })?;
                mlog!("coree: local database opened");
                (AnyDb::Local(db), None)
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
                        // Limbo 0.6.0 does not yet support direct remote client mode.
                        // We use a temporary file replica as a workaround. The TempDir is kept
                        // alive on Db so the directory is cleaned up when the connection closes.
                        let tmp = tempfile::Builder::new()
                            .prefix("coree-remote-direct-")
                            .tempdir()
                            .context("Failed to create temp dir for direct-mode replica")?;
                        let path = tmp.path().join("memory.db");
                        let path_str = path.to_str().context("temp path is not valid UTF-8")?;
                        let db = open_replica_with_recovery(path_str, &path, url, token, can_purge).await?;
                        (AnyDb::Synced(db), Some(tmp))
                    }
                    RemoteMode::Replica => {
                        let path = config.db_path();
                        ensure_parent_dir(&path)?;
                        let path_str = path
                            .to_str()
                            .context("replica DB path is not valid UTF-8")?;
                        let db =
                            open_replica_with_recovery(path_str, path.as_ref(), url, token, can_purge).await?;
                        (AnyDb::Synced(db), None)
                    }
                }
            }
        };

        let conn = match &any_db {
            AnyDb::Local(db) => db.connect().context("Failed to connect to database")?,
            AnyDb::Synced(db) => db
                .connect()
                .await
                .context("Failed to connect to synced database")?,
        };

        // journal_mode=WAL returns a result row so must use query(), not execute_batch().
        // execute_batch() cannot handle rows and returns "unexpected row during execution".
        conn.query("PRAGMA journal_mode=WAL", ())
            .await
            .context("Failed to set WAL mode")?;
        // busy_timeout: replica mode needs this too — the background sync thread can briefly
        // lock the WAL, and without a retry budget store_memories fails with "database is locked".
        conn.execute_batch("PRAGMA busy_timeout=5000;")
            .await
            .context("Failed to set busy_timeout")?;

        tracing::debug!(elapsed_ms = t.elapsed().as_millis(), "Db::open");
        Ok(Self {
            conn,
            handle: any_db,
            temp_dir,
        })
    }
}

async fn open_replica_with_recovery(
    path_str: &str,
    path: &Path,
    url: &str,
    token: &str,
    can_purge: bool,
) -> Result<turso::sync::Database> {
    mlog!("coree: opening replica (local cache {path_str}, remote {url})");

    let build = || async {
        let mut last_err = None;
        // GOTCHA: In Turso 0.6.0-pre.22, 'experimental_multiprocess_wal' is NOT available
        // for synced replicas. Only one process can have a replica open at a time.
        // We use a high retry count (20 attempts / 5s) to handle process handovers during
        // quick restarts, allowing the previous process time to fully exit and release the lock.
        for i in 0..20 {
            let t = std::time::Instant::now();
            match turso::sync::Builder::new_remote(path_str)
                .with_remote_url(url)
                .with_auth_token(token)
                .build()
                .await
            {
                Ok(db) => {
                    mlog!(
                        "coree: replica build succeeded on attempt {} ({} ms)",
                        i + 1,
                        t.elapsed().as_millis()
                    );
                    return Ok(db);
                }
                Err(e) => {
                    // Log every attempt's actual error - these errors (e.g. the Limbo
                    // sync-engine 'sqlite_sequence already exists' replay bug) are the
                    // primary signal for diagnosing replica-open failures.
                    mlog!("coree: replica build attempt {} failed: {e:#}", i + 1);
                    last_err = Some(e);
                    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                }
            }
        }
        Err(match last_err {
            Some(e) => anyhow::anyhow!("Failed to build replica after 20 attempts: {e}"),
            None => anyhow::anyhow!("Failed to build replica after 20 attempts: unknown error"),
        })
    };

    let try_sync = |db: turso::sync::Database| async move {
        let mut last_err = None;
        let t_sync = std::time::Instant::now();
        for i in 0..5 {
            match db.pull().await {
                Ok(_) => {
                    mlog!(
                        "coree: replica pull succeeded on attempt {} ({} ms)",
                        i + 1,
                        t_sync.elapsed().as_millis()
                    );
                    return Ok(db);
                }
                Err(e) => {
                    mlog!("coree: replica pull attempt {} failed: {e:#}", i + 1);
                    last_err = Some(e);
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                }
            }
        }
        Err(match last_err {
            Some(e) => anyhow::anyhow!("Failed to sync replica after 5 attempts: {e}"),
            None => anyhow::anyhow!("Failed to sync replica after 5 attempts: unknown error"),
        })
    };

    let try_open = || async {
        let db = build().await?;
        let db = try_sync(db).await?;
        // Checkpoint the freshly-pulled WAL frames into the main db file BEFORE
        // any connection is opened against this replica. The connection's schema
        // cache is built at connect() time; if we connect first and checkpoint
        // afterwards, that cache is snapshotted against the pre-checkpoint (empty
        // or stale) catalog and never refreshed, so Limbo then reports a false
        // "no such table" for tables that ARE materialized (e.g. schema_migrations
        // during startup migrations). Checkpointing here guarantees connect() sees
        // the materialized catalog. Non-fatal: a failed checkpoint should not block
        // open; the periodic background sync will checkpoint again.
        let t_cp = std::time::Instant::now();
        match db.checkpoint().await {
            Ok(_) => mlog!(
                "coree: replica checkpoint after sync complete ({} ms)",
                t_cp.elapsed().as_millis()
            ),
            Err(e) => mlog!("coree: replica checkpoint after sync failed (non-fatal): {e:#}"),
        }
        Ok::<_, anyhow::Error>(db)
    };

    match try_open().await {
        Ok(db) => return Ok(db),
        Err(e) => {
            if !can_purge {
                return Err(anyhow::anyhow!(
                    "Failed to open replica (serve may still be running — stop it first): {e:#}"
                ));
            }
            mlog!(
                "coree: CRITICAL: replica open failed ({e:#}). Backing up and clearing local replica files to force full resync..."
            );
        }
    }

    backup_replica_files(path)?;

    mlog!("coree: retrying replica open after backing up local files (full resync)...");
    let result = try_open().await.with_context(|| {
        format!(
            "Failed to open replica DB at {} (after recovery attempt)",
            path.display()
        )
    });
    match &result {
        Ok(_) => mlog!("coree: replica recovery succeeded after full resync"),
        Err(e) => mlog!("coree: replica recovery FAILED after full resync: {e:#}"),
    }
    result
}

/// Move the local replica DB and its sidecar files (`-wal`, `-info`, `-changes`,
/// `-wal-revert`, ...) into a timestamped `backup-stale-<ts>` subdirectory rather
/// than deleting them. The replica is only a local cache of the authoritative
/// remote, so a re-pull reconstructs it; backing up (not deleting) keeps a
/// recoverable copy in case the remote is ever in doubt.
pub fn backup_replica_files(path: &Path) -> Result<()> {
    let parent = path.parent().unwrap_or(std::path::Path::new("."));
    let prefix = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_string();

    let backup_dir = parent.join(format!(
        "backup-stale-{}",
        chrono::Utc::now().format("%Y%m%d-%H%M%S")
    ));

    let entries = std::fs::read_dir(parent)
        .with_context(|| format!("Failed to read dir {}", parent.display()))?;
    let mut moved = 0u32;
    for entry in entries.flatten() {
        // Only move plain files matching the replica prefix; never recurse into
        // (or move) existing backup-stale-* directories.
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with(&prefix) {
            // Create the backup dir lazily, only once there is something to move.
            std::fs::create_dir_all(&backup_dir).with_context(|| {
                format!("Failed to create backup dir {}", backup_dir.display())
            })?;
            let dest = backup_dir.join(&name);
            match std::fs::rename(entry.path(), &dest) {
                Ok(()) => {
                    moved += 1;
                    mlog!("coree: backed up stale replica file {name_str} -> {}", dest.display());
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => {
                    return Err(e).with_context(|| {
                        format!(
                            "Failed to move {} to {}",
                            entry.path().display(),
                            dest.display()
                        )
                    });
                }
            }
        }
    }
    if moved > 0 {
        mlog!(
            "coree: backed up {moved} stale replica file(s) to {}",
            backup_dir.display()
        );
    } else {
        mlog!("coree: no local replica files to back up (none matched {prefix})");
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
