use anyhow::Result;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

use super::git;
use super::parser::{self, Chunk, Lang};
use super::store::{CodeIndexStore, IndexedChunk, IndexedFile};
use crate::embed::Embedder;

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
    store: Arc<dyn CodeIndexStore>,
    embedder: Arc<Mutex<Embedder>>,
    git_history: bool,
    extra_excludes: Vec<String>,
) -> Result<IndexResult> {
    let mut result = IndexResult::default();

    crate::mlog!("index: scanning {}", project_root.display());
    let files: Vec<(PathBuf, Lang)> = {
        let root = project_root.clone();
        let excludes = extra_excludes.clone();
        tokio::task::spawn_blocking(move || collect_files(&root, &excludes)).await??
    };

    result.files_scanned = files.len();
    crate::mlog!("index: found {} indexable files", files.len());

    let cpu_count = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(2);
    let semaphore = Arc::new(tokio::sync::Semaphore::new(cpu_count.max(1)));
    let total = files.len();

    for (i, (file_path, lang)) in files.into_iter().enumerate() {
        let _permit = semaphore.clone().acquire_owned().await?;
        let store = Arc::clone(&store);
        let embedder = Arc::clone(&embedder);
        let project_root = project_root.clone();

        match index_file(
            &project_root,
            &file_path,
            &lang,
            &store,
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
            Ok(_) => {}
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

/// Remove all index data for a deleted file.
pub(crate) async fn remove_file(
    store: &Arc<dyn CodeIndexStore>,
    project_root: &Path,
    file_path: &Path,
) -> Result<()> {
    let rel_path = file_path
        .strip_prefix(project_root)
        .unwrap_or(file_path)
        .to_string_lossy()
        .to_string();
    store.remove_file(&rel_path).await
}

/// Index a single file. Returns number of new/updated chunks stored (0 = unchanged).
pub(crate) async fn index_file(
    project_root: &Path,
    file_path: &Path,
    lang: &Lang,
    store: &Arc<dyn CodeIndexStore>,
    embedder: &Arc<Mutex<Embedder>>,
    git_history: bool,
) -> Result<usize> {
    let source = tokio::fs::read_to_string(file_path).await?;
    let content_hash = sha256(&source);

    let rel_path = file_path
        .strip_prefix(project_root)
        .unwrap_or(file_path)
        .to_string_lossy()
        .to_string();

    let stored_hash = store.file_content_hash(&rel_path).await?;
    if stored_hash.as_deref() == Some(&content_hash) {
        return Ok(0);
    }

    let source_clone = source.clone();
    let rel_path_clone = rel_path.clone();
    let lang_copy = *lang;
    let chunks: Vec<Chunk> = tokio::task::spawn_blocking(move || {
        parser::parse_file(&source_clone, &rel_path_clone, &lang_copy)
    })
    .await?;

    // Fetch git commits
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

    let language = lang.name().to_string();

    let mut indexed_chunks = Vec::new();
    for chunk in &chunks {
        let embed_text = parser::build_embed_text(chunk, &rel_path);
        let chunk_hash = sha256(&embed_text);

        let embedding = {
            let mut e = embedder.lock().await;
            e.embed(&embed_text)
                .map_err(|e| anyhow::anyhow!("embed failed: {e}"))?
        };

        indexed_chunks.push(IndexedChunk {
            chunk: chunk.clone(),
            content_hash: chunk_hash,
            embedding,
        });
    }

    let indexed = IndexedFile {
        rel_path,
        content_hash,
        language,
        churn_count,
        hotspot_score,
        commits,
        chunks: indexed_chunks,
    };

    let chunk_count = indexed.chunks.len();
    store.replace_file(indexed).await?;

    Ok(chunk_count)
}

fn sha256(data: &str) -> String {
    let mut h = Sha256::new();
    h.update(data.as_bytes());
    hex::encode(h.finalize())
}

pub(crate) fn glob_match(pattern: &str, path: &str) -> bool {
    let pattern = pattern.replace('\\', "/");
    let path = path.replace('\\', "/");
    if let Some(prefix) = pattern
        .strip_suffix("/**")
        .or_else(|| pattern.strip_suffix('/'))
    {
        return path.starts_with(&format!("{prefix}/")) || path == prefix;
    }
    if let Some(suffix) = pattern.strip_prefix("**/") {
        return path == suffix || path.ends_with(&format!("/{suffix}"));
    }
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
        assert!(glob_match("vendor/**", "vendor/foo/bar.rs"));
        assert!(glob_match("vendor/", "vendor/foo/bar.rs"));
        assert!(glob_match("vendor/**", "vendor"));
        assert!(!glob_match("vendor/**", "src/vendor/foo.rs"));
    }

    #[test]
    fn glob_double_star_prefix() {
        assert!(glob_match("**/generated", "src/generated"));
        assert!(glob_match("**/generated", "generated"));
        assert!(!glob_match("**/generated", "src/generated/foo.rs"));
    }

    #[test]
    fn glob_windows_backslash_normalised() {
        assert!(glob_match("vendor/**", "vendor\\foo\\bar.rs"));
        assert!(glob_match("src/main.rs", "src\\main.rs"));
    }

    #[test]
    fn glob_no_partial_prefix_match() {
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

    #[test]
    fn collect_files_nested_repo_gitignore_star() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = dir.path();

        std::fs::create_dir(root.join(".git")).unwrap();
        std::fs::write(root.join(".gitignore"), "*\n!.gitignore\n!.coree.toml\n").unwrap();
        std::fs::write(root.join(".coree.toml"), "[project]\nid = \"test\"\n").unwrap();

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

        assert!(paths.contains(&".coree.toml".to_string()), "outer whitelisted file missing: {paths:?}");
        assert!(paths.iter().any(|p| p.contains("coree/src/main.rs")), "nested main.rs missing: {paths:?}");
        assert!(paths.iter().any(|p| p.contains("coree/src/lib.rs")), "nested lib.rs missing: {paths:?}");
        assert!(!paths.iter().any(|p| p.contains(".git")), ".git dir should not be indexed: {paths:?}");
    }

    #[test]
    fn collect_files_nested_respects_child_gitignore() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = dir.path();

        std::fs::create_dir(root.join(".git")).unwrap();
        std::fs::write(root.join(".gitignore"), "*\n!.gitignore\n").unwrap();

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
