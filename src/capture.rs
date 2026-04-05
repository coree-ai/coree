use anyhow::Result;
use chrono::Utc;
use std::io::Read;
use uuid::Uuid;

use crate::{config::Config, db::Db, migrations, project_id};

/// Tools whose outputs are worth capturing for later review.
/// Read-only tools (Read, Glob, Grep) are excluded - they carry no memory signal.
const CAPTURE_TOOLS: &[&str] = &["Write", "Edit", "MultiEdit", "Bash"];

pub async fn run(project_override: Option<String>) -> Result<()> {
    let mut buf = String::new();
    std::io::stdin().read_to_string(&mut buf)?;
    let buf = buf.trim();
    if buf.is_empty() {
        return Ok(());
    }

    let data: serde_json::Value = serde_json::from_str(buf)?;

    let tool_name = match data.get("tool_name").and_then(|v| v.as_str()) {
        Some(t) => t,
        None => return Ok(()),
    };

    if !CAPTURE_TOOLS.contains(&tool_name) {
        return Ok(());
    }

    let summary = extract_summary(tool_name, &data);

    // Use cwd from hook payload if available, fall back to process cwd.
    let cwd = data
        .get("cwd")
        .and_then(|v| v.as_str())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    let config = Config::load(&cwd)?;
    let db = Db::open(&config).await?;
    let conn = db.conn;
    migrations::run(&conn).await?;

    let pid = project_override
        .unwrap_or_else(|| project_id::resolve(&cwd, config.memory.project_id.as_deref()));

    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO raw_captures (id, project_id, captured_at, tool_name, summary, raw_data) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        libsql::params![id, pid, now, tool_name.to_string(), summary, buf.to_string()],
    )
    .await?;

    Ok(())
}

fn extract_summary(tool_name: &str, data: &serde_json::Value) -> String {
    let input = &data["tool_input"];
    match tool_name {
        "Write" => {
            let path = input.get("file_path").and_then(|v| v.as_str()).unwrap_or("?");
            format!("Created {path}")
        }
        "Edit" | "MultiEdit" => {
            let path = input.get("file_path").and_then(|v| v.as_str()).unwrap_or("?");
            format!("Edited {path}")
        }
        "Bash" => {
            let cmd = input.get("command").and_then(|v| v.as_str()).unwrap_or("?");
            let cmd_line = cmd.lines().next().unwrap_or(cmd).trim();
            let cmd_short = if cmd_line.len() > 60 { &cmd_line[..60] } else { cmd_line };
            let output = data
                .get("tool_response")
                .and_then(|r| r.get("output"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let out_line = output.lines().map(|l| l.trim()).find(|l| !l.is_empty()).unwrap_or("");
            if out_line.is_empty() {
                format!("Ran: {cmd_short}")
            } else {
                let out_short = if out_line.len() > 50 { &out_line[..50] } else { out_line };
                format!("Ran: {cmd_short} -> {out_short}")
            }
        }
        other => other.to_string(),
    }
}
