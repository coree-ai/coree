use anyhow::Result;
use std::env;
use std::io::{IsTerminal, Read};

use crate::{config::Config, db::Db, embed::Embedder, migrations, project_id, retrieve};

pub async fn run(
    inject_type: &str,
    project_override: Option<String>,
    query_override: Option<String>,
    limit: usize,
    budget: usize,
) -> Result<()> {
    let cwd = env::current_dir()?;
    let config = Config::load(&cwd)?;

    let db = Db::open(&config).await?;
    let conn = db.conn;
    migrations::run(&conn).await?;

    let pid = project_override.unwrap_or_else(|| {
        project_id::resolve(&cwd, config.memory.project_id.as_deref())
    });

    match inject_type {
        "session" => run_session(&conn, &pid, limit, budget).await,
        _ => {
            let query = resolve_prompt_query(query_override);
            if query.is_empty() {
                return Ok(());
            }
            let mut embedder = Embedder::load()?;
            run_prompt(&conn, &mut embedder, &query, &pid, limit, budget).await
        }
    }
}

async fn run_prompt(
    conn: &libsql::Connection,
    embedder: &mut Embedder,
    query: &str,
    project_id: &str,
    limit: usize,
    budget: usize,
) -> Result<()> {
    let results = retrieve::search(conn, embedder, query, project_id, limit).await?;
    if results.is_empty() {
        return Ok(());
    }
    let output = format_compact(&results);
    print_within_budget(&output, budget);
    Ok(())
}

async fn run_session(
    conn: &libsql::Connection,
    project_id: &str,
    limit: usize,
    budget: usize,
) -> Result<()> {
    let results = retrieve::list(conn, project_id, None, &[], limit).await?;
    if results.is_empty() {
        return Ok(());
    }
    let output = format_compact(&results);
    print_within_budget(&output, budget);
    Ok(())
}

/// Resolve the query for prompt injection.
/// Precedence: --query flag > $CLAUDE_USER_PROMPT env > stdin JSON {"prompt":"..."} > stdin raw
fn resolve_prompt_query(query_override: Option<String>) -> String {
    if let Some(q) = query_override {
        return q;
    }

    if let Ok(v) = env::var("CLAUDE_USER_PROMPT")
        && !v.is_empty()
    {
        return v;
    }

    // Try reading from stdin if it's not a tty
    if !std::io::stdin().is_terminal() {
        let mut buf = String::new();
        if std::io::stdin().read_to_string(&mut buf).is_ok() && !buf.trim().is_empty() {
            // Gemini CLI sends {"prompt": "..."}
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&buf)
                && let Some(p) = v.get("prompt").and_then(|p| p.as_str())
            {
                return p.to_string();
            }
            return buf.trim().to_string();
        }
    }

    String::new()
}

fn format_compact(results: &[retrieve::CompactResult]) -> String {
    let mut out = format!("--- Memory Context ({} results) ---\n", results.len());
    for r in results {
        let date = r.created_at.get(..10).unwrap_or(&r.created_at);
        out.push_str(&format!(
            "[{:<18}] {}  {}  {}\n",
            r.memory_type, r.id, date, r.title
        ));
    }
    out.push_str("---\n");
    out
}

fn print_within_budget(output: &str, budget: usize) {
    if output.len() <= budget {
        print!("{output}");
    } else {
        // Truncate at last newline within budget
        let truncated = &output[..budget];
        if let Some(pos) = truncated.rfind('\n') {
            print!("{}", &truncated[..pos]);
            println!("\n[memso: output truncated to fit budget]");
        } else {
            print!("{truncated}");
        }
    }
}
