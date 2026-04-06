use std::env;
use std::path::Path;

/// Derive the project ID for the given working directory.
///
/// Precedence:
/// 1. `$MEMSO_PROJECT` environment variable
/// 2. `project_id` in `.memso.toml` (passed in as `config_value`)
/// 3. Basename of the working directory
///
/// Always returns a non-empty string. Logs the resolved value to stderr.
pub fn resolve(cwd: &Path, config_value: Option<&str>) -> String {
    if let Ok(v) = env::var("MEMSO_PROJECT")
        && !v.is_empty()
    {
        return log_and_return(v, "MEMSO_PROJECT env var");
    }

    if let Some(v) = config_value
        && !v.is_empty()
    {
        return log_and_return(v.to_string(), ".memso.toml");
    }

    let basename = cwd
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    log_and_return(basename, "CWD basename")
}

fn log_and_return(id: String, source: &str) -> String {
    eprintln!("memso: project_id = {id:?} (from {source})");
    id
}
