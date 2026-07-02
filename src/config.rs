use anyhow::{Context, Result, bail};
use figment::{
    Figment,
    providers::{Env, Format, Toml},
};
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum StorageMode {
    /// Stored in platform data dir, keyed by project path. Zero config required.
    #[default]
    Managed,
    /// Stored at `local_path` (relative to project root if not absolute).
    Local,
    /// libsql remote backend. `remote_mode = direct` (default) or `replica`.
    Remote,
    /// Subsystem entirely disabled. No DB opened. No tools available.
    Disabled,
}

#[derive(Debug, Clone, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RemoteMode {
    #[default]
    Direct,
    Replica,
}

#[derive(Clone, Deserialize, Default)]
pub struct StorageConfig {
    #[serde(default)]
    pub mode: StorageMode,
    /// Override the managed-mode base directory.
    #[serde(default)]
    pub managed_path: Option<PathBuf>,
    /// Path for local mode (relative to project root if not absolute).
    #[serde(default)]
    pub local_path: Option<PathBuf>,
    /// Only relevant when mode = remote. Defaults to direct.
    #[serde(default)]
    pub remote_mode: RemoteMode,
    #[serde(default)]
    pub remote_url: Option<String>,
    #[serde(default)]
    pub remote_auth_token: Option<String>,
}

impl std::fmt::Debug for StorageConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StorageConfig")
            .field("mode", &self.mode)
            .field("remote_mode", &self.remote_mode)
            .field("remote_url", &self.remote_url)
            .field(
                "remote_auth_token",
                &self.remote_auth_token.as_deref().map(|_| "[REDACTED]"),
            )
            .finish()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct MemoryConfig {
    #[serde(flatten)]
    pub storage: StorageConfig,
    #[serde(default = "default_cross_session_threshold")]
    pub cross_session_notification_threshold: f32,
}

fn default_cross_session_threshold() -> f32 {
    0.8
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            storage: StorageConfig::default(),
            cross_session_notification_threshold: 0.8,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum IndexBackend {
    #[default]
    Turso,
    Sqlite,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IndexConfig {
    #[serde(default)]
    pub backend: IndexBackend,
    #[serde(flatten)]
    pub storage: StorageConfig,
    /// Include git commit history for churn analysis.
    #[serde(default = "default_true")]
    pub git_history: bool,
    /// Additional glob patterns to exclude from indexing (merged with built-in excludes).
    #[serde(default)]
    pub exclude: Vec<String>,
}

fn default_true() -> bool {
    true
}

impl Default for IndexConfig {
    fn default() -> Self {
        Self {
            backend: IndexBackend::default(),
            storage: StorageConfig::default(),
            git_history: true,
            exclude: vec![],
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
struct ProjectRootConfig {
    #[serde(default)]
    project_root: Option<PathBuf>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Config {
    /// Project identifier. Affects both memory query scoping and managed path keying.
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub memory: MemoryConfig,
    #[serde(default)]
    pub index: IndexConfig,
    /// Path the project config (`.coree.toml`) was loaded from, if any.
    /// Used only for `toml_edit` writes -- not for path derivation.
    #[serde(skip)]
    pub source_path: Option<PathBuf>,
    /// Root directory of the project. All paths are derived from this.
    /// Set by `Config::load()`: explicit `COREE__PROJECT_ROOT` env var or `project_root` in config,
    /// otherwise `.coree.toml` parent -> nearest `.git/` ancestor -> CWD.
    #[serde(default)]
    project_root: Option<PathBuf>,
}

/// Returns true if the value should be treated as if the env var was never set.
/// Handles empty strings (hosts that expand unset ${VAR} to "") and unexpanded
/// literals like "${COREE_MODEL_DIR}" (hosts like OpenClaw that do not expand).
pub fn is_unset_env_value(value: &str) -> bool {
    if value.is_empty() {
        return true;
    }
    if value.starts_with("${") && value.ends_with('}') && !value.contains(' ') {
        return true;
    }
    false
}

/// Returns the env var value, or None if it should be treated as unset.
pub fn env_var_or_unset(key: &str) -> Option<String> {
    match std::env::var(key) {
        Ok(v) if !is_unset_env_value(&v) => Some(v),
        _ => None,
    }
}

/// Env provider for COREE__ vars that skips any variable whose value is empty
/// or an unexpanded literal like "${COREE__MEMORY__MODE}".
/// Prevents Gemini CLI's ${UNSET_VAR} -> "" expansion from overriding .coree.toml values.
fn coree_env() -> Env {
    const PREFIX: &str = "COREE__";
    let valid: std::collections::HashSet<String> = std::env::vars()
        .filter(|(k, v)| k.starts_with(PREFIX) && !is_unset_env_value(v))
        .map(|(k, _)| k)
        .collect();
    Env::raw()
        .filter_map(move |k| valid.contains(k.as_str()).then(|| k[PREFIX.len()..].into()))
        .split("__")
}

impl Config {
    /// Returns the project root. Always `Some` after `Config::load()`.
    pub fn project_root(&self) -> &Path {
        self.project_root
            .as_deref()
            .expect("project_root not initialized; use Config::load()")
    }

    /// Load config with layered precedence: defaults < file < env vars.
    ///
    /// File resolution: walk up from `start_dir` looking for `.coree.toml`,
    /// then fall back to the global config at `$XDG_CONFIG_HOME/coree/config.toml`.
    ///
    /// Env var mapping: `COREE__<SECTION>__<FIELD>` overrides `section.field`.
    /// Double underscore separates nesting levels; single underscore is part of the name.
    ///   COREE__PROJECT_ROOT              -> project_root (overrides config file discovery start dir)
    ///   COREE__MEMORY__MODE              -> memory.mode        (managed|local|remote|disabled)
    ///   COREE__MEMORY__REMOTE_MODE       -> memory.remote_mode (direct|replica)
    ///   COREE__MEMORY__REMOTE_URL        -> memory.remote_url
    ///   COREE__MEMORY__REMOTE_AUTH_TOKEN -> memory.remote_auth_token
    ///   COREE__PROJECT_ID                -> project_id
    pub fn load(start_dir: &Path) -> Result<Self> {
        let global_config = global_config_path().filter(|p| p.exists());

        // First pass: extract project_root from global config + env vars so it can be
        // used as the start directory for .coree.toml discovery.
        let bootstrap_root = configured_project_root({
            let mut fig = Figment::new();
            if let Some(ref path) = global_config {
                fig = fig.merge(Toml::file(path));
            }
            fig.merge(coree_env())
        })?;
        let effective_start = bootstrap_root.as_deref().unwrap_or(start_dir);
        let project_config = find_project_config(effective_start);

        // Second pass: full config load with the discovered project config file.
        let mut fig = Figment::new();
        if let Some(ref path) = global_config {
            fig = fig.merge(Toml::file(path));
        }
        if let Some(ref path) = project_config {
            fig = fig.merge(Toml::file(path));
        }
        fig = fig.merge(coree_env());

        let mut cfg: Config = fig.extract().context("Failed to load configuration")?;
        cfg.source_path = project_config;
        if cfg.project_root.is_none() {
            cfg.project_root = Some(find_project_root(
                effective_start,
                cfg.source_path.as_deref(),
            ));
        } else {
            validate_project_root(cfg.project_root.as_deref().unwrap())?;
        }
        cfg.normalize_index_storage();
        Ok(cfg)
    }

    /// The code index is a rebuildable, per-checkout, write-heavy derivative of the
    /// working tree; `project_id` is not a meaningful discriminator for code, so it
    /// must never be shared via a remote database. Remote storage is also not
    /// implemented for the index — it is always opened as a local file — so a remote
    /// configuration would be silently ignored. Coerce any remote index storage to
    /// managed-local and warn, leaving the memory storage untouched.
    fn normalize_index_storage(&mut self) {
        let s = &self.index.storage;
        let configured_remote = s.mode == StorageMode::Remote
            || s.remote_url.is_some()
            || s.remote_auth_token.is_some();
        if configured_remote {
            eprintln!(
                "[coree] Remote/shared storage is not supported for the code index \
                 (it is rebuildable per-checkout); using managed-local storage for the \
                 index instead. Memory storage is unaffected."
            );
            self.index.storage.mode = StorageMode::Managed;
            self.index.storage.remote_mode = RemoteMode::default();
            self.index.storage.remote_url = None;
            self.index.storage.remote_auth_token = None;
        }
    }

    /// Resolved DB path for the current memory storage mode.
    ///
    /// - Managed: `{data_dir}/coree/managed/{encoded_path}/memory.db`
    /// - Local:   `{local_path}` (relative to project root if not absolute)
    /// - Remote/Replica: managed path (or local_path if set)
    /// - Remote/Direct:  managed path (parent used for serve.lock/ready/crash.log)
    pub fn db_path(&self) -> PathBuf {
        let s = &self.memory.storage;
        match s.mode {
            StorageMode::Managed | StorageMode::Disabled => self
                .managed_base(s)
                .join(encode_project_path(self.project_root()))
                .join("memory.db"),
            StorageMode::Local => self.resolve_local_path(s, ".coree/memory.db"),
            StorageMode::Remote => match s.remote_mode {
                RemoteMode::Replica => {
                    if s.local_path.is_some() {
                        self.resolve_local_path(s, ".coree/memory.replica.db")
                    } else {
                        self.managed_base(s)
                            .join(encode_project_path(self.project_root()))
                            .join("memory.replica.db")
                    }
                }
                RemoteMode::Direct => {
                    // No real local DB; parent dir used for serve.lock/ready/crash.log.
                    self.managed_base(s)
                        .join(encode_project_path(self.project_root()))
                        .join("memory.remote.db")
                }
            },
        }
    }

    /// Path to the lock file held exclusively by `coree serve` for its entire lifetime.
    pub fn serve_lock_path(&self) -> PathBuf {
        self.db_path()
            .parent()
            .map(|p| p.join("serve.lock"))
            .unwrap_or_else(|| PathBuf::from("serve.lock"))
    }

    /// Path to the lock file used for file-watcher leader election across processes.
    pub fn index_watcher_lock_path(&self) -> PathBuf {
        self.index_db_path()
            .parent()
            .map(|p| p.join("index.watcher.lock"))
            .unwrap_or_else(|| PathBuf::from("index.watcher.lock"))
    }

    /// Path to the ready file written by `coree serve` once the DB and embedder are loaded.
    pub fn serve_ready_path(&self) -> PathBuf {
        self.db_path()
            .parent()
            .map(|p| p.join("serve.ready"))
            .unwrap_or_else(|| PathBuf::from("serve.ready"))
    }

    /// Unix socket path for the local IPC channel between `coree serve` and `coree request`.
    /// On Windows the socket path is converted to a named pipe name in serve/request code.
    pub fn serve_socket_path(&self) -> PathBuf {
        self.db_path()
            .parent()
            .map(|p| p.join("coree.sock"))
            .unwrap_or_else(|| PathBuf::from("coree.sock"))
    }

    /// Windows named pipe name derived from the socket path (unique per data directory).
    #[cfg(windows)]
    pub fn serve_pipe_name(&self) -> String {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.serve_socket_path().hash(&mut h);
        format!(r"\\.\pipe\coree-{:016x}", h.finish())
    }

    /// Always returns the effective local DB path regardless of remote mode.
    /// Used by `remote enable` as the source/seed database.
    pub fn local_db_path(&self) -> PathBuf {
        let s = &self.memory.storage;
        match s.mode {
            StorageMode::Local => self.resolve_local_path(s, ".coree/memory.db"),
            _ => self
                .managed_base(s)
                .join(encode_project_path(self.project_root()))
                .join("memory.db"),
        }
    }

    /// Path to the code intelligence index database. Filename is per-backend
    /// (index.db for turso, index-sqlite.db for sqlite) so switching backends
    /// never converts or reuses files.
    ///
    /// - Managed: `{data_dir}/coree/managed/{encoded_path}/{filename}`
    /// - Local:   `{local_path}` (relative to project root if not absolute)
    pub fn index_db_path(&self) -> PathBuf {
        let s = &self.index.storage;
        let filename = match self.index.backend {
            IndexBackend::Turso => "index.db",
            IndexBackend::Sqlite => "index-sqlite.db",
        };
        match s.mode {
            StorageMode::Managed | StorageMode::Disabled | StorageMode::Remote => self
                .managed_base(s)
                .join(encode_project_path(self.project_root()))
                .join(filename),
            StorageMode::Local => self.resolve_local_path(s, &format!(".coree/{filename}")),
        }
    }

    fn managed_base(&self, s: &StorageConfig) -> PathBuf {
        s.managed_path.clone().unwrap_or_else(|| {
            dirs::data_dir()
                .unwrap_or_else(|| {
                    dirs::home_dir()
                        .unwrap_or_default()
                        .join(".local")
                        .join("share")
                })
                .join("coree")
                .join("managed")
        })
    }

    fn resolve_local_path(&self, s: &StorageConfig, default: &str) -> PathBuf {
        let p = s.local_path.as_deref().unwrap_or(Path::new(default));
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            self.project_root().join(p)
        }
    }
}

/// Encode an absolute project path into a flat directory name.
/// Mirrors Claude Code's path-encoding convention: replace `/` with `-`.
/// `/home/user/myproject` -> `-home-user-myproject`
fn encode_project_path(path: &Path) -> String {
    path.to_string_lossy().replace('/', "-")
}

fn find_project_config(start: &Path) -> Option<PathBuf> {
    let mut dir = start.to_path_buf();
    loop {
        let candidate = dir.join(".coree.toml");
        if candidate.exists() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Determine the project root directory for anchoring paths.
///
/// Walk-up chain:
/// 1. Parent of the project `.coree.toml` (if found)
/// 2. Nearest ancestor directory containing `.git/`
/// 3. `start_dir` as final fallback (handles global-config-only and no-git cases)
fn find_project_root(start_dir: &Path, project_config: Option<&Path>) -> PathBuf {
    if let Some(parent) = project_config.and_then(|p| p.parent()) {
        return parent.to_path_buf();
    }
    let mut dir = start_dir.to_path_buf();
    loop {
        if dir.join(".git").exists() {
            return dir;
        }
        if !dir.pop() {
            break;
        }
    }
    start_dir.to_path_buf()
}

fn global_config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("coree").join("config.toml"))
}

fn configured_project_root(fig: Figment) -> Result<Option<PathBuf>> {
    let Some(path) = fig
        .extract::<ProjectRootConfig>()
        .context("Failed to load project_root configuration")?
        .project_root
    else {
        return Ok(None);
    };
    validate_project_root(&path)?;
    Ok(Some(path))
}

fn validate_project_root(path: &Path) -> Result<()> {
    if !path.is_absolute() {
        bail!("project_root must be an absolute path: {}", path.display());
    }
    if !path.is_dir() {
        bail!(
            "project_root must point to an existing directory: {}",
            path.display()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn db_path_managed_mode() {
        let cfg = Config {
            project_root: Some(PathBuf::from("/some/project")),
            ..Default::default()
        };
        let path = cfg.db_path();
        assert!(path.ends_with("memory.db"));
        assert!(path.to_string_lossy().contains("coree"));
        assert!(path.to_string_lossy().contains("-some-project"));
    }

    #[test]
    fn normalize_index_storage_coerces_remote_to_managed() {
        let mut cfg = Config {
            project_root: Some(PathBuf::from("/some/project")),
            index: IndexConfig {
                storage: StorageConfig {
                    mode: StorageMode::Remote,
                    remote_url: Some("libsql://shared.turso.io".to_string()),
                    remote_auth_token: Some("secret".to_string()),
                    ..Default::default()
                },
                ..Default::default()
            },
            // Memory remote config must survive normalization untouched.
            memory: MemoryConfig {
                storage: StorageConfig {
                    mode: StorageMode::Remote,
                    remote_url: Some("libsql://mem.turso.io".to_string()),
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };
        cfg.normalize_index_storage();

        assert_eq!(cfg.index.storage.mode, StorageMode::Managed);
        assert!(cfg.index.storage.remote_url.is_none());
        assert!(cfg.index.storage.remote_auth_token.is_none());
        // index_db_path is a local managed path, never remote.
        assert!(cfg.index_db_path().ends_with("index.db"));

        // Memory storage is untouched.
        assert_eq!(cfg.memory.storage.mode, StorageMode::Remote);
        assert_eq!(
            cfg.memory.storage.remote_url.as_deref(),
            Some("libsql://mem.turso.io")
        );
    }

    #[test]
    fn normalize_index_storage_leaves_managed_and_disabled() {
        for mode in [
            StorageMode::Managed,
            StorageMode::Local,
            StorageMode::Disabled,
        ] {
            let mut cfg = Config {
                project_root: Some(PathBuf::from("/some/project")),
                index: IndexConfig {
                    storage: StorageConfig {
                        mode: mode.clone(),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                ..Default::default()
            };
            cfg.normalize_index_storage();
            assert_eq!(cfg.index.storage.mode, mode);
        }
    }

    #[test]
    fn db_path_local_mode() {
        let cfg = Config {
            project_root: Some(PathBuf::from("/some/project")),
            memory: MemoryConfig {
                storage: StorageConfig {
                    mode: StorageMode::Local,
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(
            cfg.db_path(),
            PathBuf::from("/some/project/.coree/memory.db")
        );
    }

    #[test]
    fn db_path_local_mode_explicit_path() {
        let cfg = Config {
            project_root: Some(PathBuf::from("/some/project")),
            memory: MemoryConfig {
                storage: StorageConfig {
                    mode: StorageMode::Local,
                    local_path: Some(PathBuf::from("custom/memory.db")),
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(
            cfg.db_path(),
            PathBuf::from("/some/project/custom/memory.db")
        );
    }

    #[test]
    fn local_db_path_managed_returns_managed_path() {
        let cfg = Config {
            project_root: Some(PathBuf::from("/some/project")),
            ..Default::default()
        };
        let path = cfg.local_db_path();
        assert!(path.ends_with("memory.db"));
        assert!(path.to_string_lossy().contains("coree"));
    }

    #[test]
    fn find_project_root_uses_project_config_parent() {
        let root = find_project_root(
            Path::new("/some/subdir"),
            Some(Path::new("/some/project/.coree.toml")),
        );
        assert_eq!(root, PathBuf::from("/some/project"));
    }

    #[test]
    fn find_project_root_falls_back_to_start_dir() {
        // Create an isolated tree with .git at the root so the walk stops there
        // rather than escaping into the ambient environment where /tmp/.git may exist.
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(temp.path().join(".git")).unwrap();
        let start = temp.path().join("nested").join("subdir");
        std::fs::create_dir_all(&start).unwrap();
        let root = find_project_root(&start, None);
        assert_eq!(root, temp.path());
    }

    #[test]
    fn project_root_can_be_configured_from_project_file() {
        let temp = tempfile::tempdir().unwrap();
        let actual_root = temp.path().join("actual");
        let configured_root = temp.path().join("configured");
        std::fs::create_dir_all(&actual_root).unwrap();
        std::fs::create_dir_all(&configured_root).unwrap();
        std::fs::write(
            actual_root.join(".coree.toml"),
            format!("project_root = \"{}\"\n", configured_root.display()),
        )
        .unwrap();

        let cfg = Config::load(&actual_root).unwrap();

        assert_eq!(cfg.project_root(), configured_root.as_path());
        assert_eq!(cfg.source_path, Some(actual_root.join(".coree.toml")));
    }

    #[test]
    fn encode_project_path_replaces_slashes() {
        assert_eq!(
            encode_project_path(Path::new("/home/user/project")),
            "-home-user-project"
        );
        assert_eq!(
            encode_project_path(Path::new("/some/project")),
            "-some-project"
        );
    }

    // --- coree_env() / empty-var filtering tests ---

    // Verify that an empty COREE__ env var is excluded from the Figment layer entirely,
    // so it cannot override a value set in a lower-priority source (e.g. a TOML file).
    #[test]
    fn empty_env_var_does_not_override_toml() {
        figment::Jail::expect_with(|jail| {
            jail.create_file(".coree.toml", "[memory]\nmode = \"remote\"\n")?;
            jail.set_env("COREE__MEMORY__MODE", "");

            let cfg = Config::load(jail.directory()).unwrap();
            assert_eq!(cfg.memory.storage.mode, StorageMode::Remote);
            Ok(())
        });
    }

    // A non-empty env var must still win over TOML (standard Figment precedence).
    #[test]
    fn non_empty_env_var_overrides_toml() {
        figment::Jail::expect_with(|jail| {
            jail.create_file(".coree.toml", "[memory]\nmode = \"remote\"\n")?;
            jail.set_env("COREE__MEMORY__MODE", "local");

            let cfg = Config::load(jail.directory()).unwrap();
            assert_eq!(cfg.memory.storage.mode, StorageMode::Local);
            Ok(())
        });
    }

    // An unset var (absent from env) must also leave the TOML value intact.
    #[test]
    fn absent_env_var_preserves_toml() {
        figment::Jail::expect_with(|jail| {
            jail.clear_env();
            jail.create_file(".coree.toml", "[memory]\nmode = \"local\"\n")?;

            let cfg = Config::load(jail.directory()).unwrap();
            assert_eq!(cfg.memory.storage.mode, StorageMode::Local);
            Ok(())
        });
    }

    // Empty Option<String> env vars (e.g. auth token) should come through as None.
    #[test]
    fn empty_env_var_yields_none_for_optional_string() {
        figment::Jail::expect_with(|jail| {
            jail.create_file(".coree.toml", "")?;
            jail.set_env("COREE__MEMORY__REMOTE_AUTH_TOKEN", "");

            let cfg = Config::load(jail.directory()).unwrap();
            assert_eq!(cfg.memory.storage.remote_auth_token, None);
            Ok(())
        });
    }

    // A non-empty auth token env var must be passed through.
    #[test]
    fn non_empty_auth_token_env_var_is_set() {
        figment::Jail::expect_with(|jail| {
            jail.create_file(".coree.toml", "")?;
            jail.set_env("COREE__MEMORY__REMOTE_AUTH_TOKEN", "mytoken");

            let cfg = Config::load(jail.directory()).unwrap();
            assert_eq!(
                cfg.memory.storage.remote_auth_token.as_deref(),
                Some("mytoken")
            );
            Ok(())
        });
    }

    #[test]
    fn is_unset_env_value_empty() {
        assert!(is_unset_env_value(""));
    }

    #[test]
    fn is_unset_env_value_literal_var() {
        assert!(is_unset_env_value("${COREE_MODEL_DIR}"));
        assert!(is_unset_env_value("${COREE__MEMORY__MODE}"));
        assert!(is_unset_env_value("${FOO}"));
    }

    #[test]
    fn is_unset_env_value_literal_with_digits() {
        assert!(is_unset_env_value("${VAR_2}"));
    }

    #[test]
    fn is_unset_env_value_literal_with_spaces_is_not_unset() {
        assert!(!is_unset_env_value("${SOME VAR}"));
    }

    #[test]
    fn is_unset_env_value_legit_value() {
        assert!(!is_unset_env_value("/home/user/.cache/coree/models"));
        assert!(!is_unset_env_value("token123"));
        assert!(!is_unset_env_value("1"));
    }

    #[test]
    fn is_unset_env_value_partial_pattern_not_unset() {
        assert!(!is_unset_env_value("${incomplete"));
        assert!(!is_unset_env_value("noprefix}"));
        assert!(!is_unset_env_value("pre${VAR}post"));
    }

    // COREE__ vars with literal ${...} values are dropped so TOML is preserved.
    #[test]
    fn coree_env_filters_literal_var_values() {
        figment::Jail::expect_with(|jail| {
            jail.set_env("COREE__MEMORY__MODE", "${COREE__MEMORY__MODE}");
            jail.create_file(".coree.toml", "[memory]\nmode = \"local\"\n")?;

            let cfg = Config::load(jail.directory()).unwrap();
            assert_eq!(
                cfg.memory.storage.mode,
                StorageMode::Local,
                "TOML value should be preserved when env var is a literal ${{...}}"
            );
            Ok(())
        });
    }

    // COREE__ vars with empty values are dropped so TOML is preserved (existing behavior).
    #[test]
    fn coree_env_filters_empty_values() {
        figment::Jail::expect_with(|jail| {
            jail.set_env("COREE__MEMORY__MODE", "");
            jail.create_file(".coree.toml", "[memory]\nmode = \"local\"\n")?;

            let cfg = Config::load(jail.directory()).unwrap();
            assert_eq!(cfg.memory.storage.mode, StorageMode::Local);
            Ok(())
        });
    }
}
