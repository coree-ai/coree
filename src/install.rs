use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::path::PathBuf;

const MCP_SERVER_NAME: &str = "memso";
const HOOK_SESSION_CMD: &str = "memso inject --type session";
const HOOK_PROMPT_CMD: &str = "memso inject --type prompt --budget 8000";

pub struct InstallResult {
    pub mcp_added: bool,
    pub session_hook_added: bool,
    pub prompt_hook_added: bool,
    pub settings_path: PathBuf,
}

pub fn run(dry_run: bool) -> Result<InstallResult> {
    let path = settings_path()?;
    let mut root = read_or_empty(&path)?;

    let mcp_added = ensure_mcp_server(&mut root);
    let session_hook_added = ensure_hook(&mut root, "SessionStart", HOOK_SESSION_CMD);
    let prompt_hook_added = ensure_hook(&mut root, "UserPromptSubmit", HOOK_PROMPT_CMD);

    let changed = mcp_added || session_hook_added || prompt_hook_added;

    if changed && !dry_run {
        write_settings(&path, &root)?;
    }

    Ok(InstallResult {
        mcp_added,
        session_hook_added,
        prompt_hook_added,
        settings_path: path,
    })
}

/// Ensure `mcpServers.memso` exists with the correct command.
/// Returns true if a change was made.
fn ensure_mcp_server(root: &mut Value) -> bool {
    let servers = root
        .as_object_mut()
        .unwrap()
        .entry("mcpServers")
        .or_insert_with(|| json!({}));

    if let Some(existing) = servers.get(MCP_SERVER_NAME) {
        // Already present - check it points to the right command
        if existing.get("command").and_then(|v| v.as_str()) == Some("memso") {
            return false;
        }
    }

    servers[MCP_SERVER_NAME] = json!({
        "type": "stdio",
        "command": "memso",
        "args": ["serve"]
    });
    true
}

/// Ensure a hook entry with the given command exists under the given event.
/// Returns true if a change was made.
fn ensure_hook(root: &mut Value, event: &str, command: &str) -> bool {
    let hooks_map = root
        .as_object_mut()
        .unwrap()
        .entry("hooks")
        .or_insert_with(|| json!({}));

    let event_list = hooks_map
        .as_object_mut()
        .unwrap()
        .entry(event)
        .or_insert_with(|| json!([]));

    let list = event_list.as_array_mut().unwrap();

    // Check if any existing entry already contains this command
    let already_present = list.iter().any(|entry| {
        // Flat format: {"matcher": "", "hooks": [{"type": "command", "command": "..."}]}
        if let Some(inner) = entry.get("hooks").and_then(|h| h.as_array())
            && inner.iter().any(|h| h.get("command").and_then(|c| c.as_str()) == Some(command))
        {
            return true;
        }
        // Simple format: {"command": "..."}
        if entry.get("command").and_then(|c| c.as_str()) == Some(command) {
            return true;
        }
        false
    });

    if already_present {
        return false;
    }

    list.push(json!({
        "matcher": "",
        "hooks": [
            {
                "type": "command",
                "command": command
            }
        ]
    }));
    true
}

fn settings_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("Could not determine home directory")?;
    Ok(home.join(".claude").join("settings.json"))
}

fn read_or_empty(path: &PathBuf) -> Result<Value> {
    if !path.exists() {
        return Ok(json!({}));
    }
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    if text.trim().is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_str(&text)
        .with_context(|| format!("Failed to parse JSON in {}", path.display()))
}

fn write_settings(path: &PathBuf, value: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(value)?;
    std::fs::write(path, text + "\n")
        .with_context(|| format!("Failed to write {}", path.display()))
}
