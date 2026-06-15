use anyhow::Result;
use chrono::Utc;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

use super::git;
use super::parser::{self, Chunk, Lang};
use crate::embed::{self, Embedder};

/// Directories and patterns always skipped regardless of .gitignore.
const ALWAYS_EXCLUDE: &[&str] = &[
    ".git",
    "target",
    "node_modules",
    "dist",
    "build",
    "__pycache__",
    ".venv",
    "vendor",
    ".mypy_cache",
    ".devenv",
];

pub(crate) fn is_excluded(path: &Path) -> bool {
    for component in path.components() {
        let name = component.as_os_str().to_string_lossy();
        if ALWAYS_EXCLUDE.iter().any(|e| name == *e) {
            return true;
        }
        // Skip generated/lock files
        if name.ends_with(".min.js")
            || name.ends_with(".min.css")
            || name == "package-lock.json"
            || name == "yarn.lock"
            || name.ends_with(".lock")
            || name.ends_with(".sum")
        {
            return true;
        }
    }
    false
}

/// Result of a full index run.
#[derive(Debug, Default)]
pub struct IndexResult {
    pub files_scanned: usize,
    pub files_indexed: usize,
    pub chunks_stored: usize,
}

/// Run a full index of the project root.
/// Uses file content hashes to skip unchanged files.
/// Runs in a Tokio blocking task per file to avoid starving the async runtime.
pub async fn run(
    project_root: PathBuf,
    conn: Arc<turso::Connection>,
    embedder: Arc<Mutex<Embedder>>,
    git_history: bool,
    extra_excludes: Vec<String>,
) -> Result<IndexResult> {
    let mut result = IndexResult::default();

    // Collect files to process (cheap, synchronous)
    crate::mlog!("index: scanning {}", project_root.display());
    let files: Vec<(PathBuf, Lang)> = {
        let root = project_root.clone();
        let excludes = extra_excludes.clone();
        tokio::task::spawn_blocking(move || collect_files(&root, &excludes)).await??
    };

    result.files_scanned = files.len();
    crate::mlog!("index: found {} indexable files", files.len());

    // Limit concurrency: parse/embed is CPU+memory intensive
    let cpu_count = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(2);
    let semaphore = Arc::new(tokio::sync::Semaphore::new(cpu_count.max(1)));
    let total = files.len();

    for (i, (file_path, lang)) in files.into_iter().enumerate() {
        let _permit = semaphore.clone().acquire_owned().await?;
        let conn = Arc::clone(&conn);
        let embedder = Arc::clone(&embedder);
        let project_root = project_root.clone();

        match index_file(
            &project_root,
            &file_path,
            &lang,
            &conn,
            &embedder,
            git_history,
        )
        .await
        {
            Ok(n) if n > 0 => {
                result.files_indexed += 1;
                result.chunks_stored += n;
                let rel = file_path.strip_prefix(&project_root).unwrap_or(&file_path);
                crate::mlog!(
                    "index: [{}/{}] {} ({} chunks)",
                    i + 1,
                    total,
                    rel.display(),
                    n
                );
            }
            Ok(_) => {} // unchanged — no log noise
            Err(e) => {
                let rel = file_path.strip_prefix(&project_root).unwrap_or(&file_path);
                crate::mlog!(
                    "index: [{}/{}] skipped {}: {e}",
                    i + 1,
                    total,
                    rel.display()
                );
            }
        }

        // Progress checkpoint every 50 files
        if (i + 1) % 50 == 0 {
            crate::mlog!(
                "index: progress {}/{} files checked, {} indexed so far",
                i + 1,
                total,
                result.files_indexed
            );
        }

        tokio::task::yield_now().await;
    }

    Ok(result)
}

/// Discover all git repository roots under `project_root`.
///
/// Walks without gitignore to find repos hidden by parent `.gitignore` rules
/// (e.g. an outer workspace `.gitignore` of `*` that prunes a nested source
/// tree). Always includes `project_root` itself as a walk root so single-repo
/// projects and loose files in non-git workspaces are still indexed.
fn discover_repo_roots(project_root: &Path) -> Result<Vec<PathBuf>> {
    let mut roots: HashSet<PathBuf> = HashSet::new();
    roots.insert(project_root.to_path_buf());

    let walker = ignore::WalkBuilder::new(project_root)
        .hidden(false)
        .git_ignore(false)
        .git_global(false)
        .git_exclude(false)
        .filter_entry(|entry| {
            let name = entry.file_name().to_string_lossy();
            if name == ".git" {
                return false;
            }
            if ALWAYS_EXCLUDE.iter().any(|e| *e == name.as_ref()) {
                return false;
            }
            true
        })
        .build();

    for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        if !entry.file_type().is_some_and(|ft| ft.is_dir()) {
            continue;
        }
        if entry.path().join(".git").exists() {
            roots.insert(entry.path().to_path_buf());
        }
    }

    Ok(roots.into_iter().collect())
}

/// Collect all indexable files under `root`, respecting each repo's own
/// `.gitignore` and built-in excludes.
///
/// Uses per-repo walks (Option A): discovers every git repo under `root`,
/// walks each with `git_ignore(true)` scoped to that repo, and deduplicates
/// paths across overlapping walks. This fixes the nested-repo data-loss bug
/// (#71) where an outer `.gitignore` of `*` pruned the entire nested source
/// tree when walking only from the project root.
fn collect_files(root: &Path, extra_excludes: &[String]) -> Result<Vec<(PathBuf, Lang)>> {
    let mut seen: HashSet<PathBuf> = HashSet::new();
    let mut files = Vec::new();

    let repo_roots = discover_repo_roots(root)?;

    for repo_root in &repo_roots {
        let walker = ignore::WalkBuilder::new(repo_root)
            .hidden(false)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .build();

        for entry in walker {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            let path = entry.path();
            if !entry.file_type().is_some_and(|ft| ft.is_file()) {
                continue;
            }
            if !seen.insert(path.to_path_buf()) {
                continue;
            }
            if is_excluded(path) {
                continue;
            }

            if !extra_excludes.is_empty() {
                let rel = path.strip_prefix(root).unwrap_or(path);
                let rel_str = rel.to_string_lossy();
                if extra_excludes.iter().any(|pat| glob_match(pat, &rel_str)) {
                    continue;
                }
            }

            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if let Some(lang) = Lang::from_extension(ext) {
                files.push((path.to_path_buf(), lang));
            }
        }
    }

    Ok(files)
}

/// Clear all rows from every index table.
///
/// Deletes dependent rows first (FK CASCADE is inert in Turso). Preserves the
/// `meta` table (schema version) so version tracking survives a rebuild.
/// Safe to call while the watcher is running — the watcher will re-index files
/// as it receives events.
pub async fn clear_all_tables(conn: &Arc<turso::Connection>) -> Result<()> {
    // FK CASCADE is inert — delete children before parents
    conn.execute("DELETE FROM index_vectors", ()).await?;
    conn.execute("DELETE FROM index_chunk_commits", ()).await?;
    conn.execute("DELETE FROM index_chunks", ()).await?;
    conn.execute("DELETE FROM index_files", ()).await?;
    conn.execute("DELETE FROM index_commits", ()).await?;
    conn.execute("DELETE FROM index_file_coupling", ()).await?;
    Ok(())
}

/// Remove all index data for a deleted file.
/// Must explicitly delete dependent rows because Turso has FK enforcement off
/// by default so ON DELETE CASCADE clauses in the schema are inert.
pub(crate) async fn remove_file(
    conn: &Arc<turso::Connection>,
    project_root: &Path,
    file_path: &Path,
) -> Result<()> {
    let rel_path = file_path
        .strip_prefix(project_root)
        .unwrap_or(file_path)
        .to_string_lossy()
        .to_string();

    // FK CASCADE is inert in Turso — delete dependent rows first
    conn.execute(
        "DELETE FROM index_vectors WHERE chunk_id IN (SELECT id FROM index_chunks WHERE file_path = ?1)",
        (rel_path.clone(),),
    )
    .await?;
    conn.execute(
        "DELETE FROM index_chunk_commits WHERE chunk_id IN (SELECT id FROM index_chunks WHERE file_path = ?1)",
        (rel_path.clone(),),
    )
    .await?;

    conn.execute(
        "DELETE FROM index_chunks WHERE file_path = ?1",
        (rel_path.clone(),),
    )
    .await?;
    conn.execute("DELETE FROM index_files WHERE path = ?1", (rel_path,))
        .await?;
    Ok(())
}

/// Recompute file coupling for `file_path` from the current index_chunk_commits table.
///
/// Bounds (D3):
/// - Commits touching >25 distinct indexed files are skipped (refactor/sweep noise).
/// - Pairs are only retained when shared_commits >= 2.
///
/// Limitations documented per issue #63:
/// - Only sees indexed files (D5: coupling/ownership ignore non-indexed files).
/// - Branch accumulation accepted (D6: INSERT OR IGNORE on commits means orphan SHAs
///   from rebase/squash/force-push linger — acceptable v1 staleness).
async fn refresh_file_coupling(conn: &Arc<turso::Connection>, file_path: &str) -> Result<()> {
    // Purge existing coupling rows for this file so we can recompute from scratch
    conn.execute(
        "DELETE FROM index_file_coupling WHERE file_a = ?1 OR file_b = ?1",
        (file_path.to_string(),),
    )
    .await?;

    // Recompute coupling for this file, skipping noisy commits (D3: >25 distinct
    // indexed files = refactor/sweep noise) and retaining only pairs with >=2 shared
    // commits. canonical ordering: file_a < file_b.
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
        (file_path.to_string(),),
    )
    .await?;

    Ok(())
}

/// Index a single file. Returns number of new/updated chunks stored (0 = unchanged).
pub(crate) async fn index_file(
    project_root: &Path,
    file_path: &Path,
    lang: &Lang,
    conn: &Arc<turso::Connection>,
    embedder: &Arc<Mutex<Embedder>>,
    git_history: bool,
) -> Result<usize> {
    let source = tokio::fs::read_to_string(file_path).await?;
    let content_hash = sha256(&source);

    // Relative path for storage (deterministic across machines)
    let rel_path = file_path
        .strip_prefix(project_root)
        .unwrap_or(file_path)
        .to_string_lossy()
        .to_string();

    // Check if file hash has changed
    let mut rows = conn
        .query(
            "SELECT content_hash FROM index_files WHERE path = ?1",
            (rel_path.clone(),),
        )
        .await?;
    let stored_hash: Option<String> = rows.next().await?.map(|r| r.get(0)).transpose()?;

    if stored_hash.as_deref() == Some(&content_hash) {
        return Ok(0); // unchanged
    }

    // FK CASCADE is inert in Turso — delete dependent rows first
    conn.execute(
        "DELETE FROM index_vectors WHERE chunk_id IN (SELECT id FROM index_chunks WHERE file_path = ?1)",
        (rel_path.clone(),),
    )
    .await?;
    conn.execute(
        "DELETE FROM index_chunk_commits WHERE chunk_id IN (SELECT id FROM index_chunks WHERE file_path = ?1)",
        (rel_path.clone(),),
    )
    .await?;

    // Delete old chunks for this file
    conn.execute(
        "DELETE FROM index_chunks WHERE file_path = ?1",
        (rel_path.clone(),),
    )
    .await?;

    // Parse in blocking thread (tree-sitter is synchronous, CPU-bound)
    let source_clone = source.clone();
    let rel_path_clone = rel_path.clone();
    let lang_name = lang.name().to_string();
    let lang_copy = *lang;
    let chunks: Vec<Chunk> = tokio::task::spawn_blocking(move || {
        parser::parse_file(&source_clone, &rel_path_clone, &lang_copy)
    })
    .await?;

    if chunks.is_empty() {
        // File is indexed but has no extractable symbols (update hash to avoid re-scanning)
        upsert_file_hash(conn, &rel_path, &content_hash).await?;
        return Ok(0);
    }

    // Fetch git commits once and reuse for churn_count, hotspot_score, commit storage, and chunk linking.
    let (commits, hotspot_score) = if git_history {
        let abs_path = file_path.to_path_buf();
        let stats = tokio::task::spawn_blocking(move || {
            let git_root = git::resolve_git_root(&abs_path);
            let git_rel = abs_path
                .strip_prefix(&git_root)
                .unwrap_or(&abs_path)
                .to_string_lossy()
                .to_string();
            git::file_commits_with_stats(&git_root, &git_rel, 10)
        })
        .await?;
        let score = git::compute_hotspot_score(&stats);
        (stats, score)
    } else {
        (vec![], 0.0)
    };
    let churn_count = commits.len() as i64;

    // Store commit records for history search
    for commit in &commits {
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

    let now = Utc::now().to_rfc3339();
    let model_id = crate::embed::model_id();
    let mut stored = 0usize;

    for chunk in &chunks {
        let embed_text = parser::build_embed_text(chunk, &rel_path);
        let chunk_hash = sha256(&embed_text);

        // Embed with the shared embedder
        let embedding = {
            let mut e = embedder.lock().await;
            e.embed(&embed_text)
                .map_err(|e| anyhow::anyhow!("embed failed: {e}"))?
        };
        let blob = embed::floats_to_blob(&embedding);

        let chunk_id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO index_chunks \
             (id, file_path, symbol_name, qualified_name, symbol_kind, signature, \
              doc_comment, body_preview, line_start, line_end, language, \
              churn_count, hotspot_score, indexed_at, content_hash) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
            (
                chunk_id.clone(),
                rel_path.clone(),
                chunk.symbol_name.clone(),
                chunk.qualified_name.clone(),
                chunk.symbol_kind.clone(),
                chunk.signature.clone(),
                chunk.doc_comment.clone(),
                chunk.body_preview.clone(),
                chunk.line_start as i64,
                chunk.line_end as i64,
                lang_name.clone(),
                churn_count,
                hotspot_score,
                now.clone(),
                chunk_hash.clone(),
            ),
        )
        .await?;

        conn.execute(
            "INSERT OR REPLACE INTO index_vectors (chunk_id, embed_model, embedding) \
             VALUES (?1, ?2, ?3)",
            (chunk_id.clone(), model_id.clone(), blob),
        )
        .await?;

        for commit in &commits {
            let _ = conn
                .execute(
                    "INSERT OR IGNORE INTO index_chunk_commits (chunk_id, commit_sha) \
                 VALUES (?1, ?2)",
                    (chunk_id.clone(), commit.sha.clone()),
                )
                .await;
        }

        stored += 1;
    }

    // Incremental file coupling: delete old rows for this file then recompute
    // from current index_chunk_commits. Only sees indexed files (D5 limitation).
    // Branch-accumulation accepted (D6): INSERT OR IGNORE on index_chunk_commits
    // means rebase/squash orphan SHAs linger — acceptable v1 staleness.
    if git_history {
        let _ = refresh_file_coupling(conn, &rel_path).await;
    }

    upsert_file_hash(conn, &rel_path, &content_hash).await?;

    Ok(stored)
}

async fn upsert_file_hash(conn: &Arc<turso::Connection>, path: &str, hash: &str) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT OR REPLACE INTO index_files (path, content_hash, indexed_at) \
         VALUES (?1, ?2, ?3)",
        (path.to_string(), hash.to_string(), now),
    )
    .await?;
    Ok(())
}

fn sha256(data: &str) -> String {
    let mut h = Sha256::new();
    h.update(data.as_bytes());
    hex::encode(h.finalize())
}

/// Simple glob match for exclude patterns. Handles `**` and `*` wildcards.
pub(crate) fn glob_match(pattern: &str, path: &str) -> bool {
    let pattern = pattern.replace('\\', "/");
    let path = path.replace('\\', "/");
    // "vendor/**" or "vendor/" → anything under that directory
    if let Some(prefix) = pattern
        .strip_suffix("/**")
        .or_else(|| pattern.strip_suffix('/'))
    {
        return path.starts_with(&format!("{prefix}/")) || path == prefix;
    }
    // "**/foo" → any path component named foo
    if let Some(suffix) = pattern.strip_prefix("**/") {
        return path == suffix || path.ends_with(&format!("/{suffix}"));
    }
    // Exact match
    path == pattern
}

#[cfg(test)]
mod tests {
    use super::{glob_match, is_excluded};
    use std::path::Path;

    // --- is_excluded ---

    #[test]
    fn excluded_builtin_dirs() {
        assert!(is_excluded(Path::new("target/release/coree")));
        assert!(is_excluded(Path::new("node_modules/react/index.js")));
        assert!(is_excluded(Path::new(".git/objects/pack/foo")));
        assert!(is_excluded(Path::new("__pycache__/foo.pyc")));
        assert!(is_excluded(Path::new(".venv/lib/site-packages/foo.py")));
        assert!(is_excluded(Path::new("vendor/github.com/foo/bar.go")));
    }

    #[test]
    fn excluded_generated_files() {
        assert!(is_excluded(Path::new("src/bundle.min.js")));
        assert!(is_excluded(Path::new("dist/style.min.css")));
        assert!(is_excluded(Path::new("Cargo.lock")));
        assert!(is_excluded(Path::new("go.sum")));
        assert!(is_excluded(Path::new("package-lock.json")));
        assert!(is_excluded(Path::new("yarn.lock")));
    }

    #[test]
    fn excluded_devenv_dir() {
        assert!(is_excluded(Path::new(".devenv/shell-abc.sh")));
        assert!(is_excluded(Path::new(".devenv/state/go/pkg/foo.go")));
    }

    #[test]
    fn not_excluded_normal_source() {
        assert!(!is_excluded(Path::new("src/main.rs")));
        assert!(!is_excluded(Path::new("src/index/parser.rs")));
        assert!(!is_excluded(Path::new("tests/db.rs")));
        assert!(!is_excluded(Path::new("README.md")));
    }

    // --- glob_match ---

    #[test]
    fn glob_exact_match() {
        assert!(glob_match("src/main.rs", "src/main.rs"));
        assert!(!glob_match("src/main.rs", "src/lib.rs"));
    }

    #[test]
    fn glob_dir_prefix() {
        // "vendor/" or "vendor/**" matches anything inside vendor/
        assert!(glob_match("vendor/**", "vendor/foo/bar.rs"));
        assert!(glob_match("vendor/", "vendor/foo/bar.rs"));
        assert!(glob_match("vendor/**", "vendor"));
        assert!(!glob_match("vendor/**", "src/vendor/foo.rs"));
    }

    #[test]
    fn glob_double_star_prefix() {
        // "**/foo" matches any path ending in /foo or equal to foo
        assert!(glob_match("**/generated", "src/generated"));
        assert!(glob_match("**/generated", "generated"));
        assert!(!glob_match("**/generated", "src/generated/foo.rs"));
    }

    #[test]
    fn glob_windows_backslash_normalised() {
        // Windows paths with backslashes should match forward-slash patterns
        assert!(glob_match("vendor/**", "vendor\\foo\\bar.rs"));
        assert!(glob_match("src/main.rs", "src\\main.rs"));
    }

    #[test]
    fn glob_no_partial_prefix_match() {
        // "vendor/**" must not match a file whose path starts with "vendor" but
        // is a different directory (e.g. "vendor_utils/foo.rs")
        assert!(!glob_match("vendor/**", "vendor_utils/foo.rs"));
    }

    // --- discover_repo_roots ---

    #[test]
    fn discover_roots_single_repo() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::create_dir(dir.path().join(".git")).unwrap();

        let roots = super::discover_repo_roots(dir.path()).unwrap();
        assert_eq!(roots.len(), 1);
        assert_eq!(&roots[0], dir.path());
    }

    #[test]
    fn discover_roots_nested_repo() {
        let dir = tempfile::TempDir::new().unwrap();
        let nested = dir.path().join("nested");
        std::fs::create_dir_all(nested.join(".git")).unwrap();

        let roots = super::discover_repo_roots(dir.path()).unwrap();
        assert_eq!(roots.len(), 2);
        assert!(roots.contains(&dir.path().to_path_buf()));
        assert!(roots.contains(&nested));
    }

    /// Regression test for #71: outer `.gitignore` of `*` prunes nested source.
    /// The per-repo walk must discover the nested git repo and index its files.
    #[test]
    fn collect_files_nested_repo_gitignore_star() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = dir.path();

        // Outer repo with .gitignore = * (ignore everything except whitelist)
        std::fs::create_dir(root.join(".git")).unwrap();
        std::fs::write(root.join(".gitignore"), "*\n!.gitignore\n!.coree.toml\n").unwrap();
        std::fs::write(root.join(".coree.toml"), "[project]\nid = \"test\"\n").unwrap();

        // Nested git repo (hidden by outer .gitignore *)
        let nested = root.join("coree");
        std::fs::create_dir_all(nested.join(".git")).unwrap();
        std::fs::create_dir_all(nested.join("src")).unwrap();
        std::fs::write(nested.join("src").join("main.rs"), "fn main() {}\n").unwrap();
        std::fs::write(nested.join("src").join("lib.rs"), "pub fn add(a: i32, b: i32) -> i32 { a + b }\n").unwrap();

        let files = super::collect_files(root, &[]).unwrap();
        let paths: Vec<String> = files
            .iter()
            .map(|(p, _)| p.strip_prefix(root).unwrap().to_string_lossy().to_string())
            .collect();

        // Outer whitelisted file
        assert!(paths.contains(&".coree.toml".to_string()), "outer whitelisted file missing: {paths:?}");

        // Nested source files (the bug: these were pruned before the fix)
        assert!(paths.iter().any(|p| p.contains("coree/src/main.rs")), "nested main.rs missing: {paths:?}");
        assert!(paths.iter().any(|p| p.contains("coree/src/lib.rs")), "nested lib.rs missing: {paths:?}");

        // Nested repo .git directory itself is excluded (is_excluded catches .git dirs)
        assert!(!paths.iter().any(|p| p.contains(".git")), ".git dir should not be indexed: {paths:?}");
    }

    /// Child repo `.gitignore` (e.g. `target/`) is still honoured by the
    /// per-repo walk — the parent's blanket `*` no longer prunes, but the
    /// child's own excludes still apply.
    #[test]
    fn collect_files_nested_respects_child_gitignore() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = dir.path();

        // Outer repo with gitignore = * (ignore everything)
        std::fs::create_dir(root.join(".git")).unwrap();
        std::fs::write(root.join(".gitignore"), "*\n!.gitignore\n").unwrap();

        // Nested repo with its own .gitignore excluding target/
        let nested = root.join("mylib");
        std::fs::create_dir_all(nested.join(".git")).unwrap();
        std::fs::create_dir_all(nested.join("target")).unwrap();
        std::fs::create_dir_all(nested.join("src")).unwrap();
        std::fs::write(nested.join(".gitignore"), "target/\n").unwrap();
        std::fs::write(nested.join("src").join("lib.rs"), "pub fn foo() {}\n").unwrap();
        std::fs::write(nested.join("target").join("output.txt"), "build artifact\n").unwrap();

        let files = super::collect_files(root, &[]).unwrap();
        let paths: Vec<String> = files
            .iter()
            .map(|(p, _)| p.strip_prefix(root).unwrap().to_string_lossy().to_string())
            .collect();

        assert!(paths.iter().any(|p| p.contains("mylib/src/lib.rs")), "nested source missing: {paths:?}");
        assert!(!paths.iter().any(|p| p.contains("mylib/target")), "child target/ should be excluded: {paths:?}");
    }
}
