use anyhow::Result;
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

use super::git;
use super::indexer;
use super::store::CodeIndexStore;
use crate::embed::Embedder;

const RETRY_INTERVAL: Duration = Duration::from_secs(2);
const DRAIN_INTERVAL: Duration = Duration::from_millis(500);

/// Spawn the watcher leader-election loop as a background task.
/// The task loops indefinitely: tries to acquire the lock, runs the watcher,
/// releases the lock, waits 2s, repeats. Never panics or crashes serve.
pub fn start(
    lock_path: PathBuf,
    project_root: PathBuf,
    store: Arc<dyn CodeIndexStore>,
    embedder: Arc<Mutex<Embedder>>,
    git_history: bool,
    extra_excludes: Vec<String>,
) {
    tokio::spawn(async move {
        loop {
            let lock_file = match try_acquire_lock(&lock_path) {
                Some(f) => f,
                None => {
                    tokio::time::sleep(RETRY_INTERVAL).await;
                    continue;
                }
            };

            crate::mlog!("coree: file watcher acquired leader lock");

            if let Err(e) = run_watchers(
                &project_root,
                Arc::clone(&store),
                Arc::clone(&embedder),
                git_history,
                &extra_excludes,
            )
            .await
            {
                crate::mlog!("coree: file watcher stopped: {e:#}");
            }

            drop(lock_file);
            crate::mlog!(
                "coree: file watcher released leader lock, retrying in {}s",
                RETRY_INTERVAL.as_secs()
            );
            tokio::time::sleep(RETRY_INTERVAL).await;
        }
    });
}

fn try_acquire_lock(path: &Path) -> Option<std::fs::File> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .ok()?;
    match f.try_lock() {
        Ok(()) => Some(f),
        _ => None,
    }
}

async fn run_watchers(
    project_root: &Path,
    store: Arc<dyn CodeIndexStore>,
    embedder: Arc<Mutex<Embedder>>,
    git_history: bool,
    extra_excludes: &[String],
) -> Result<()> {
    let (src_tx, src_rx) = std::sync::mpsc::channel();
    let (git_tx, git_rx) = std::sync::mpsc::channel();

    let mut src_watcher: RecommendedWatcher = notify::recommended_watcher(move |ev| {
        let _ = src_tx.send(ev);
    })?;
    src_watcher.watch(project_root, RecursiveMode::Recursive)?;

    let mut git_watcher: Option<RecommendedWatcher> = None;
    let git_dir = project_root.join(".git");
    if git_dir.exists() {
        let mut w: RecommendedWatcher = notify::recommended_watcher(move |ev| {
            let _ = git_tx.send(ev);
        })?;
        w.watch(&git_dir, RecursiveMode::NonRecursive)?;
        git_watcher = Some(w);
    }

    let store_src = Arc::clone(&store);
    let emb_src = Arc::clone(&embedder);
    let root_src = project_root.to_path_buf();
    let excludes_src = extra_excludes.to_vec();

    let store_git = Arc::clone(&store);
    let root_git = project_root.to_path_buf();

    let src_handle = tokio::spawn(async move {
        let mut dirty: HashSet<PathBuf> = HashSet::new();
        let mut interval = tokio::time::interval(DRAIN_INTERVAL);
        loop {
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
                    if !excludes_src.is_empty() {
                        let rel = path.strip_prefix(&root_src).unwrap_or(&path);
                        let rel_str = rel.to_string_lossy();
                        if excludes_src
                            .iter()
                            .any(|p| indexer::glob_match(p, &rel_str))
                        {
                            continue;
                        }
                    }
                    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                    if let Some(lang) = crate::index::parser::Lang::from_extension(ext) {
                        if path.exists() {
                            match indexer::index_file(
                                &root_src, &path, &lang, &store_src, &emb_src, false,
                            )
                            .await
                            {
                                Ok(n) if n > 0 => {
                                    let rel = path.strip_prefix(&root_src).unwrap_or(&path);
                                    crate::mlog!("coree: reindexed {} ({n} chunks)", rel.display());
                                }
                                Ok(_) => {}
                                Err(e) => {
                                    let rel = path.strip_prefix(&root_src).unwrap_or(&path);
                                    crate::mlog!("coree: reindex error {}: {e:#}", rel.display());
                                }
                            }
                        } else if let Err(e) = indexer::remove_file(&store_src, &root_src, &path).await
                        {
                            let rel = path.strip_prefix(&root_src).unwrap_or(&path);
                            crate::mlog!("coree: remove_file error {}: {e:#}", rel.display());
                        }
                    }
                }
            }

            interval.tick().await;
        }
    });

    let git_handle = tokio::spawn(async move {
        let _git_watcher = git_watcher;
        if _git_watcher.is_none() {
            std::future::pending::<()>().await;
            return;
        }
        let mut interval = tokio::time::interval(DRAIN_INTERVAL);
        loop {
            interval.tick().await;
            loop {
                match git_rx.try_recv() {
                    Ok(Ok(event)) => {
                        if !is_commit_editmsg_event(&event) {
                            continue;
                        }
                        if let Err(e) = handle_new_commit(&root_git, &store_git, git_history).await {
                            crate::mlog!("coree: commit index error: {e:#}");
                        }
                    }
                    Ok(Err(_)) => continue,
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => return,
                }
            }
        }
    });

    let _ = &src_watcher;

    tokio::select! {
        _ = src_handle => {},
        _ = git_handle => {},
    }

    Ok(())
}

fn collect_source_paths(event: &notify::Event, root: &Path, dirty: &mut HashSet<PathBuf>) {
    match event.kind {
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {
            for path in &event.paths {
                if !path.starts_with(root.join(".git")) {
                    dirty.insert(path.clone());
                }
            }
        }
        _ => {}
    }
}

fn is_commit_editmsg_event(event: &notify::Event) -> bool {
    matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_))
        && event
            .paths
            .iter()
            .any(|p| p.file_name().is_some_and(|n| n == "COMMIT_EDITMSG"))
}

/// Update the index after a new git commit: store commit record, update churn counts,
/// link chunks to the new commit. No re-parse or re-embed.
async fn handle_new_commit(
    root: &Path,
    store: &Arc<dyn CodeIndexStore>,
    git_history: bool,
) -> Result<()> {
    if !git_history {
        return Ok(());
    }

    let commit = match tokio::task::spawn_blocking({
        let root = root.to_path_buf();
        move || git::head_commit(&root)
    })
    .await?
    {
        Some(c) => c,
        None => return Ok(()),
    };

    let changed_files = tokio::task::spawn_blocking({
        let root = root.to_path_buf();
        move || git::files_in_head_commit(&root)
    })
    .await?;

    let mut file_updates = Vec::new();
    for rel_path in &changed_files {
        let (new_count, new_hotspot) = tokio::task::spawn_blocking({
            let root = root.to_path_buf();
            let rel = rel_path.clone();
            move || {
                let stats = git::file_commits_with_stats(&root, &rel, 50);
                let score = git::compute_hotspot_score(&stats);
                (stats.len() as i64, score)
            }
        })
        .await?;
        file_updates.push((rel_path.clone(), new_count, new_hotspot));
    }

    store.record_commit(&commit, &file_updates).await?;

    crate::mlog!(
        "coree: indexed commit {} — {} files updated",
        &commit.sha[..7.min(commit.sha.len())],
        changed_files.len()
    );

    Ok(())
}
