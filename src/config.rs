use anyhow::{Context, Result};
use serde::Deserialize;
use std::env;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BackendMode {
    #[default]
    Local,
    Replica,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct BackendConfig {
    #[serde(default)]
    pub mode: BackendMode,
    pub local_path: Option<String>,
    pub remote_url: Option<String>,
    /// Supports "${ENV_VAR}" substitution
    pub auth_token: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct MemoryConfig {
    pub project_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub backend: BackendConfig,
    #[serde(default)]
    pub memory: MemoryConfig,
    /// Path this config was loaded from, if any
    #[serde(skip)]
    pub source_path: Option<PathBuf>,
}

impl Config {
    /// Load config by walking up from `start_dir`, then global, then defaults.
    pub fn load(start_dir: &Path) -> Result<Self> {
        if let Some(path) = find_project_config(start_dir) {
            let mut cfg = load_file(&path)
                .with_context(|| format!("Failed to read {}", path.display()))?;
            cfg.source_path = Some(path);
            cfg.resolve_env_vars();
            return Ok(cfg);
        }

        if let Some(path) = global_config_path()
            && path.exists()
        {
            let mut cfg = load_file(&path)
                .with_context(|| format!("Failed to read {}", path.display()))?;
            cfg.source_path = Some(path);
            cfg.resolve_env_vars();
            return Ok(cfg);
        }

        Ok(Config::default())
    }

    /// Resolve "${VAR}" references in auth_token.
    fn resolve_env_vars(&mut self) {
        if let Some(token) = &self.backend.auth_token
            && let Some(var) = token.strip_prefix("${").and_then(|s| s.strip_suffix('}'))
        {
            self.backend.auth_token = env::var(var).ok();
        }
    }

    /// Resolved local DB path, defaulting to `.memso.db` in the config file's
    /// directory, or CWD if no config file was found.
    pub fn local_db_path(&self) -> PathBuf {
        if let Some(ref p) = self.backend.local_path {
            return PathBuf::from(p);
        }
        let base = self
            .source_path
            .as_ref()
            .and_then(|p| p.parent())
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        base.join(".memso").join("memory.db")
    }
}

fn find_project_config(start: &Path) -> Option<PathBuf> {
    let mut dir = start.to_path_buf();
    loop {
        let candidate = dir.join(".memso.toml");
        if candidate.exists() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

fn global_config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("memso").join("config.toml"))
}

fn load_file(path: &Path) -> Result<Config> {
    let text = std::fs::read_to_string(path)?;
    let cfg: Config = toml::from_str(&text)?;
    Ok(cfg)
}
