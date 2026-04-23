use anyhow::Result;
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

use crate::embed::Embedder;
use super::git;
use super::indexer;

const RETRY_INTERVAL: Duration = Duration::from_secs(2);
const DRAIN_INTERVAL: Duration = Duration::from_millis(500);

/// Spawn the watcher leader-election loop as a background task.
/// The task loops indefinitely: tries to acquire the lock, runs the watcher,
/// releases the lock, waits 2s, repeats. Never panics or crashes serve.
pub fn start(
    lock_path: PathBuf,
    project_root: PathBuf,
    conn: Arc<libsql::Connection>,
    embedder: Arc<Mutex<Embedder>>,
    git_history: bool,
    extra_excludes: Vec<String>,
) {
    tokio::spawn(async move {
        loop {
            // Non-blocking attempt to become the watcher leader.
            let lock_file = match try_acquire_lock(&lock_path) {
                Some(f) => f,
                None => {
                    tokio::time::sleep(RETRY_INTERVAL).await;
                    continue;
                }
            };

            crate::mlog!("tyto: file watcher acquired leader lock");

            // Run source + commit watchers until either returns (error or shutdown).
            if let Err(e) = run_watchers(
                &project_root,
                Arc::clone(&conn),
                Arc::clone(&embedder),
                git_history,
                &extra_excludes,
            ).await {
                crate::mlog!("tyto: file watcher stopped: {e:#}");
            }

            // Release the lock explicitly before sleeping so another process can
            // pick it up immediately if we're not going to re-acquire fast enough.
            drop(lock_file);
            crate::mlog!("tyto: file watcher released leader lock, retrying in {}s", RETRY_INTERVAL.as_secs());
            tokio::time::sleep(RETRY_INTERVAL).await;
        }
    });
}

/// Try to open and exclusively lock the watcher lock file. Non-blocking.
/// Returns the open File (lock held) or None if another process holds it.
fn try_acquire_lock(path: &Path) -> Option<std::fs::File> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let f = std::fs::OpenOptions::new()
        .write(true).create(true).truncate(false)
        .open(path)
        .ok()?;
    match f.try_lock() {
        Ok(()) => Some(f),
        _ => None,
    }
}

/// Run the two watcher loops concurrently until either exits.
async fn run_watchers(
    project_root: &Path,
    conn: Arc<libsql::Connection>,
    embedder: Arc<Mutex<Embedder>>,
    git_history: bool,
    extra_excludes: &[String],
) -> Result<()> {
    let (src_tx, src_rx) = std::sync::mpsc::channel();
    let (git_tx, git_rx) = std::sync::mpsc::channel();

    // Source file watcher — watches project root recursively.
    let mut src_watcher: RecommendedWatcher = notify::recommended_watcher(move |ev| {
        let _ = src_tx.send(ev);
    })?;
    src_watcher.watch(project_root, RecursiveMode::Recursive)?;

    // Git commit watcher — watches .git/ non-recursively for COMMIT_EDITMSG.
    // If .git/ doesn't exist yet, skip the git watcher silently.
    let mut git_watcher: Option<RecommendedWatcher> = None;
    let git_dir = project_root.join(".git");
    if git_dir.exists() {
        let mut w: RecommendedWatcher = notify::recommended_watcher(move |ev| {
            let _ = git_tx.send(ev);
        })?;
        w.watch(&git_dir, RecursiveMode::NonRecursive)?;
        git_watcher = Some(w);
    }

    let conn_src = Arc::clone(&conn);
    let emb_src = Arc::clone(&embedder);
    let root_src = project_root.to_path_buf();
    let excludes_src = extra_excludes.to_vec();

    let conn_git = Arc::clone(&conn);
    let root_git = project_root.to_path_buf();

    // Source file task: interval-drain pattern.
    let src_handle = tokio::spawn(async move {
        let mut dirty: HashSet<PathBuf> = HashSet::new();
        let mut interval = tokio::time::interval(DRAIN_INTERVAL);
        loop {
            // Drain all pending events from the channel.
            loop {
                match src_rx.try_recv() {
                    Ok(Ok(event)) => collect_source_paths(&event, &root_src, &mut dirty),
                    Ok(Err(_)) | Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => return,
                }
            }

            if !dirty.is_empty() {
                let paths: Vec<PathBuf> = dirty.drain().collect();
                for path in paths {
                    if indexer::is_excluded(&path) {
                        continue;
                    }
                    // Check extra excludes.
                    if !excludes_src.is_empty() {
                        let rel = path.strip_prefix(&root_src).unwrap_or(&path);
                        let rel_str = rel.to_string_lossy();
                        if excludes_src.iter().any(|p| indexer::glob_match(p, &rel_str)) {
                            continue;
                        }
                    }
                    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                    if let Some(lang) = crate::index::parser::Lang::from_extension(ext) {
                        if path.exists() {
                            match indexer::index_file(&root_src, &path, &lang, &conn_src, &emb_src, false).await {
                                Ok(n) if n > 0 => {
                                    let rel = path.strip_prefix(&root_src).unwrap_or(&path);
                                    crate::mlog!("tyto: reindexed {} ({n} chunks)", rel.display());
                                }
                                Ok(_) => {}
                                Err(e) => {
                                    let rel = path.strip_prefix(&root_src).unwrap_or(&path);
                                    crate::mlog!("tyto: reindex error {}: {e:#}", rel.display());
                                }
                            }
                        } else {
                            // File was deleted.
                            if let Err(e) = indexer::remove_file(&conn_src, &root_src, &path).await {
                                let rel = path.strip_prefix(&root_src).unwrap_or(&path);
                                crate::mlog!("tyto: remove_file error {}: {e:#}", rel.display());
                            }
                        }
                    }
                }
            }

            interval.tick().await;
        }
    });

    let git_watcher_exists = git_watcher.is_some();

    // Git commit task: react to COMMIT_EDITMSG writes.
    let git_handle = tokio::spawn(async move {
        if !git_watcher_exists {
            // No .git dir — park until cancelled.
            std::future::pending::<()>().await;
            return;
        }
        loop {
            match git_rx.recv() {
                Ok(Ok(event)) => {
                    if !is_commit_editmsg_event(&event) {
                        continue;
                    }
                    if let Err(e) = handle_new_commit(&root_git, &conn_git, git_history).await {
                        crate::mlog!("tyto: commit index error: {e:#}");
                    }
                }
                Ok(Err(_)) => continue,
                Err(_) => return, // sender dropped
            }
        }
    });

    // Keep watcher handles alive until one task exits.
    let _ = &src_watcher;
    let _ = &git_watcher;

    // Wait for either task to finish (shouldn't happen under normal operation).
    tokio::select! {
        _ = src_handle => {},
        _ = git_handle => {},
    }

    Ok(())
}

/// Collect modified/created/deleted source file paths from a notify event.
fn collect_source_paths(event: &notify::Event, root: &Path, dirty: &mut HashSet<PathBuf>) {
    match event.kind {
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {
            for path in &event.paths {
                // Skip .git/ and other always-excluded top-level dirs.
                if !path.starts_with(root.join(".git")) {
                    dirty.insert(path.clone());
                }
            }
        }
        _ => {}
    }
}

/// Returns true if this event corresponds to COMMIT_EDITMSG being written.
fn is_commit_editmsg_event(event: &notify::Event) -> bool {
    matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_))
        && event.paths.iter().any(|p| {
            p.file_name().is_some_and(|n| n == "COMMIT_EDITMSG")
        })
}

/// Update the index after a new git commit: store commit record, update churn counts,
/// link chunks to the new commit. No re-parse or re-embed.
async fn handle_new_commit(
    root: &Path,
    conn: &libsql::Connection,
    git_history: bool,
) -> Result<()> {
    if !git_history {
        return Ok(());
    }

    let commit = match tokio::task::spawn_blocking({
        let root = root.to_path_buf();
        move || git::head_commit(&root)
    }).await? {
        Some(c) => c,
        None => return Ok(()),
    };

    // Store the commit record.
    conn.execute(
        "INSERT OR IGNORE INTO index_commits (sha, message) VALUES (?1, ?2)",
        libsql::params![commit.sha.clone(), commit.message.clone()],
    ).await?;

    // Find which files were changed in this commit.
    let changed_files = tokio::task::spawn_blocking({
        let root = root.to_path_buf();
        move || git::files_in_head_commit(&root)
    }).await?;

    for rel_path in &changed_files {
        // Update churn_count and hotspot_score for all chunks in this file.
        let (new_count, new_hotspot) = tokio::task::spawn_blocking({
            let root = root.to_path_buf();
            let rel = rel_path.clone();
            move || {
                let stats = git::file_commits_with_stats(&root, &rel, 50);
                let score = git::compute_hotspot_score(&stats);
                (stats.len() as i64, score)
            }
        }).await?;

        conn.execute(
            "UPDATE index_chunks SET churn_count = ?1, hotspot_score = ?2 WHERE file_path = ?3",
            libsql::params![new_count, new_hotspot, rel_path.clone()],
        ).await?;

        // Link all chunks in this file to the new commit.
        let mut rows = conn.query(
            "SELECT id FROM index_chunks WHERE file_path = ?1",
            libsql::params![rel_path.clone()],
        ).await?;
        while let Some(row) = rows.next().await? {
            let chunk_id: String = row.get(0)?;
            conn.execute(
                "INSERT OR IGNORE INTO index_chunk_commits (chunk_id, commit_sha) VALUES (?1, ?2)",
                libsql::params![chunk_id, commit.sha.clone()],
            ).await?;
        }
    }

    crate::mlog!(
        "tyto: indexed commit {} — {} files updated",
        &commit.sha[..7.min(commit.sha.len())],
        changed_files.len()
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::try_acquire_lock;
    use std::path::PathBuf;
    use uuid::Uuid;

    /// Return a unique path in the system temp dir. Each call returns a distinct path,
    /// so parallel tests never share a lock file.
    fn unique_lock_path() -> PathBuf {
        std::env::temp_dir().join(format!("tyto-test-watcher-{}.lock", Uuid::new_v4()))
    }

    #[test]
    fn test_acquire_when_free() {
        let path = unique_lock_path();
        let f = try_acquire_lock(&path);
        assert!(f.is_some(), "should acquire lock on an uncontested file");
    }

    #[test]
    fn test_contention_same_file() {
        let path = unique_lock_path();

        // First holder acquires the lock.
        let _holder = try_acquire_lock(&path)
            .expect("first acquire must succeed");

        // A second attempt on the same path should fail: a new open() creates a
        // separate file description, but flock(LOCK_EX) still conflicts.
        let contender = try_acquire_lock(&path);
        assert!(
            contender.is_none(),
            "second acquire must fail while first holder is alive"
        );
    }

    #[test]
    fn test_reacquire_after_drop() {
        let path = unique_lock_path();

        let holder = try_acquire_lock(&path).expect("initial acquire must succeed");

        // Confirm contention is present.
        assert!(try_acquire_lock(&path).is_none(), "must be contested while holder lives");

        // Releasing the lock (drop closes the fd and the OS releases the flock).
        drop(holder);

        // The file is now free; a new attempt must succeed.
        let new_holder = try_acquire_lock(&path);
        assert!(
            new_holder.is_some(),
            "must re-acquire after the previous holder is dropped"
        );
    }

    /// Tests the full leader-election handover scenario:
    /// Task A holds the lock, Task B waits. A releases; B must acquire.
    ///
    /// Uses oneshot channels throughout — no sleep(), no wall-clock assertions.
    /// Correctness is proven by ordering, not by timing.
    #[tokio::test]
    async fn test_lock_handover_between_tasks() {
        let path = std::sync::Arc::new(unique_lock_path());

        // Channels for precise synchronisation between tasks.
        let (a_held_tx, a_held_rx) = tokio::sync::oneshot::channel::<()>();
        let (a_release_tx, a_release_rx) = tokio::sync::oneshot::channel::<()>();
        let (b_acquired_tx, mut b_acquired_rx) = tokio::sync::oneshot::channel::<()>();

        // Task A: acquire, signal held, wait for permission to release, then drop.
        let path_a = std::sync::Arc::clone(&path);
        let task_a = tokio::spawn(async move {
            let f = try_acquire_lock(&path_a).expect("task A must acquire lock");
            a_held_tx.send(()).expect("send A-held signal");
            a_release_rx.await.expect("receive release signal");
            drop(f); // OS releases flock here
        });

        // Wait until A confirms it holds the lock before proceeding.
        a_held_rx.await.expect("receive A-held signal");

        // At this point A definitely holds the lock.
        assert!(
            try_acquire_lock(&path).is_none(),
            "lock must be contested while A holds it"
        );

        // Task B: retry loop — the same pattern used in the real watcher::start loop.
        // Retries every 10ms (fast for tests, avoids flakiness from scheduling jitter).
        let path_b = std::sync::Arc::clone(&path);
        tokio::spawn(async move {
            let mut signal = Some(b_acquired_tx);
            loop {
                if let Some(_f) = try_acquire_lock(&path_b) {
                    if let Some(tx) = signal.take() {
                        tx.send(()).expect("send B-acquired signal");
                    }
                    // Keep `_f` alive; in the real loop this would run the watcher.
                    std::future::pending::<()>().await;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        });

        // B has not acquired yet (A still holds it).
        // Use try_recv to confirm — B's channel will not be ready.
        assert!(
            b_acquired_rx.try_recv().is_err(),
            "B must not have acquired before A releases"
        );

        // Now release A. B's retry loop will pick up the lock on its next iteration.
        a_release_tx.send(()).expect("send release signal");
        task_a.await.expect("task A must complete cleanly");

        // Wait for B to signal acquisition. Give it a generous timeout (1s) —
        // with a 10ms retry interval the actual handover takes <20ms in practice.
        // The timeout only fires on a genuine bug, not scheduling noise.
        tokio::time::timeout(
            std::time::Duration::from_secs(1),
            &mut b_acquired_rx,
        )
        .await
        .expect("timed out waiting for B to acquire lock — handover failed")
        .expect("B-acquired channel closed unexpectedly");
    }
}
