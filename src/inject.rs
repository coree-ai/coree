use anyhow::Result;
use chrono::Utc;
use std::env;
use std::io::{IsTerminal, Read};

use crate::{config::Config, db::Db, migrations, project_id, retrieve};

const INSTRUCTIONS: &str = "[memso] Store every decision, discovery, gotcha, failure, and unexpected outcome. \
Err on the side of storing - use importance (0.0-1.0) to signal value, not omission. \
Failures and unexpected outcomes: type='gotcha', importance >= 0.8. \
When you find a bug: store it as gotcha before writing the fix. \
When you finish understanding a function or module: store how-it-works before moving on. \
Store inline as you work - do not defer to end of session. \
Use topic_key to upsert existing memories. \
Before starting work, and before exploring any file or module not yet examined this session: \
search memory first — check the compact index for relevant IDs and fetch with get_memories(ids); \
call search_memory for gaps not covered by the index. \
capture_note(summary) = your reasoning before/after a change, reviewed next session. \
store_memory = a fact you would want to search for today or in a future session. \
They are not interchangeable.\n\
[memso tools] store_memories(memories:[{content,type,title,[topic_key,importance,tags,facts,source,pinned]}]) | \
search_memory(query,[limit,detail]) | get_memories(ids) | \
list_memories([type,tags,limit,detail]) | capture_note(summary,[context]) | \
pin_memories(ids,pin) | delete_memories(ids)\n";

fn build_session_instructions(
    captures_path: Option<&std::path::Path>,
    memories_path: Option<&std::path::Path>,
) -> String {
    let mut steps: Vec<String> = Vec::new();

    if let Some(path) = captures_path {
        steps.push(format!(
            "READ AND SYNTHESISE CAPTURES — open this file NOW: {}\n   \
             Read ALL entries together, then store memories for any discoveries, \
             non-obvious outcomes, bugs found, or decisions made — synthesising across \
             the full set, not one memory per entry. Routine edits and builds with no \
             finding do not need a memory. Bugs/failures: type=gotcha, importance>=0.8. \
             All stored memories: source='reviewed'. If nothing is worth storing, skip \
             the store call. This step is mandatory — do not defer.",
            path.display()
        ));
    }

    if let Some(path) = memories_path {
        steps.push(format!(
            "READ MEMORY CONTENT — open this file NOW: {}\n   \
             Read it in full before responding. It contains your highest-priority memories \
             from previous sessions. The compact index below is a prioritised subset — \
             fetch relevant entries by ID with get_memories(ids) as needed during the session.",
            path.display()
        ));
    }

    if steps.is_empty() {
        return String::new();
    }

    let mut out =
        String::from("[memso] Session start — BEFORE responding, complete ALL steps:\n");
    for (i, step) in steps.iter().enumerate() {
        out.push_str(&format!("{}. {}\n", i + 1, step));
    }
    out
}

pub async fn run(
    inject_type: &str,
    project_override: Option<String>,
    query_override: Option<String>,
    limit: usize,
    budget: usize,
) -> Result<()> {
    // Stop inject needs no DB - read stop_hook_active from stdin then emit instructions.
    if inject_type == "stop" {
        return run_stop(budget);
    }

    if let Err(e) = run_inner(inject_type, project_override, query_override, limit, budget).await {
        println!("[memso ERROR: {e:#}]");
    }
    Ok(())
}

async fn run_inner(
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

    let pid = project_override
        .unwrap_or_else(|| project_id::resolve(&cwd, config.memory.project_id.as_deref()));

    match inject_type {
        "session" | "compact" => run_session(&conn, &pid, budget).await,
        _ => {
            let query = resolve_prompt_query(query_override);
            run_prompt(&conn, &query, &pid, limit, budget).await
        }
    }
}

// Uses BM25-only search (no ONNX model) to stay within the 500ms hook timeout.
// Prompt injection is best-effort context; keyword relevance is sufficient here.
// Full hybrid search is reserved for session-start where latency tolerance is higher.
async fn run_prompt(
    conn: &libsql::Connection,
    query: &str,
    project_id: &str,
    limit: usize,
    budget: usize,
) -> Result<()> {
    let mut output = INSTRUCTIONS.to_string();
    if !query.is_empty() {
        let results = retrieve::search_bm25(conn, query, project_id, limit).await?;
        if !results.is_empty() {
            output.push_str(&format_compact(&results, 0, None));
        }
    }
    print_within_budget(&output, budget);
    Ok(())
}

const STOP_INSTRUCTIONS: &str =
    "[memso] End of turn checkpoint - store anything worth keeping before moving on:\n\
- Found a bug or unexpected behavior?     -> store_memory type=gotcha importance>=0.8\n\
- Understood how a subsystem works?       -> store_memory type=how-it-works\n\
- Made a design or implementation choice? -> store_memory type=decision\n\
- Changed your approach mid-task?         -> capture_note(why)\n\
Store inline as you work - do not defer to end of session.";

// Fires on every Claude response completion. Outputs a checkpoint prompt - no DB query.
// Guards against infinite loops: if stop_hook_active is true, a Stop hook already
// ran this turn (Claude responded to the hook output), so we skip to avoid compounding.
fn run_stop(budget: usize) -> Result<()> {
    if is_stop_hook_active() {
        return Ok(());
    }
    print_within_budget(STOP_INSTRUCTIONS, budget);
    Ok(())
}

fn is_stop_hook_active() -> bool {
    if std::io::stdin().is_terminal() {
        return false;
    }
    let mut buf = String::new();
    if std::io::stdin().read_to_string(&mut buf).is_err() {
        return false;
    }
    serde_json::from_str::<serde_json::Value>(&buf)
        .ok()
        .and_then(|v| v.get("stop_hook_active").and_then(|b| b.as_bool()))
        .unwrap_or(false)
}

const FULL_CONTENT_BUDGET: usize = 30_000;

async fn run_session(
    conn: &libsql::Connection,
    project_id: &str,
    budget: usize,
) -> Result<()> {
    let pid = std::process::id();

    // Write pending captures to a temp file and mark them as presented.
    // Write before marking so a file-write failure leaves captures unPresented.
    let captures = query_pending_captures(conn, project_id).await?;
    let captures_path = if !captures.is_empty() {
        let path = std::env::temp_dir().join(format!("memso-captures-{pid}.txt"));
        std::fs::write(&path, format_captures_file(&captures))?;
        mark_captures_presented(conn, project_id).await?;
        Some(path)
    } else {
        None
    };

    // List all memories above the importance floor sorted by retention score.
    let results = retrieve::list(conn, project_id, None, &[], 500, 0.4).await?;

    // Write full memory content to a temp file.
    let mut included_in_file = 0usize;
    let memories_path = if !results.is_empty() {
        let all_ids: Vec<String> = results.iter().map(|r| r.id.clone()).collect();
        let full_memories = retrieve::get_full_batch(conn, &all_ids).await?;
        let full_map: std::collections::HashMap<String, retrieve::FullMemory> =
            full_memories.into_iter().map(|m| (m.id.clone(), m)).collect();

        let mut content = String::from(
            "[memso] Session memory content — full text for top memories.\n\
             Read in full before responding to restore context from previous sessions.\n\n",
        );
        let mut accumulated = 0usize;
        for (i, compact) in results.iter().enumerate() {
            if let Some(mem) = full_map.get(&compact.id) {
                let entry = format_full_memory(mem);
                accumulated += entry.len();
                content.push_str(&entry);
                included_in_file = i + 1;
                if accumulated >= FULL_CONTENT_BUDGET {
                    break;
                }
            }
        }

        let path = std::env::temp_dir().join(format!("memso-memories-{pid}.txt"));
        std::fs::write(&path, content)?;
        Some(path)
    } else {
        None
    };

    // Build stdout: instructions + dynamic session steps + compact index.
    // Only memories not already written to the full file appear in the compact index.
    // Kept under budget so it is always delivered inline, never saved to a file.
    let mut output = INSTRUCTIONS.to_string();
    output.push_str(&build_session_instructions(
        captures_path.as_deref(),
        memories_path.as_deref(),
    ));
    if !results.is_empty() {
        output.push_str(&format_compact(
            &results[included_in_file..],
            included_in_file,
            memories_path.as_deref(),
        ));
    }

    print_within_budget(&output, budget);
    Ok(())
}

struct PendingCapture {
    tool_name: String,
    captured_at: String,
    summary: String,
}

async fn query_pending_captures(
    conn: &libsql::Connection,
    project_id: &str,
) -> Result<Vec<PendingCapture>> {
    let mut rows = conn
        .query(
            "SELECT tool_name, captured_at, summary \
             FROM raw_captures \
             WHERE project_id = ?1 AND presented_at IS NULL \
             ORDER BY captured_at ASC",
            libsql::params![project_id.to_string()],
        )
        .await?;

    let mut captures = Vec::new();
    while let Some(row) = rows.next().await? {
        captures.push(PendingCapture {
            tool_name: row.get(0)?,
            captured_at: row.get(1)?,
            summary: row.get(2)?,
        });
    }
    Ok(captures)
}

async fn mark_captures_presented(conn: &libsql::Connection, project_id: &str) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE raw_captures SET presented_at = ?1 \
         WHERE project_id = ?2 AND presented_at IS NULL",
        libsql::params![now, project_id.to_string()],
    )
    .await?;
    Ok(())
}

fn format_captures_file(captures: &[PendingCapture]) -> String {
    let mut out = format!(
        "[memso] Pending Review — {} captures from previous session activity.\n\
         Read ALL entries together. Store memories only for discoveries and non-obvious outcomes:\n\
         - Bugs/failures: type=gotcha, importance>=0.8, source='reviewed'\n\
         - Other findings: appropriate type, source='reviewed'\n\
         - Routine edits/builds with no finding: no memory needed\n\n\
         --- Captures ---\n",
        captures.len()
    );
    for c in captures {
        let date = c.captured_at.get(..10).unwrap_or(&c.captured_at);
        out.push_str(&format!("[{:<12}] {}  {}\n", c.tool_name, date, c.summary));
    }
    out.push_str("---\n");
    out
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

fn format_full_memory(mem: &retrieve::FullMemory) -> String {
    let date = mem.created_at.get(..10).unwrap_or(&mem.created_at);
    let mut out = format!(
        "[{} {:.2}] {}\nid: {} | {}\n",
        mem.memory_type, mem.importance, mem.title, mem.id, date
    );
    if let Some(tags) = &mem.tags {
        let parsed: Vec<String> = serde_json::from_str(tags).unwrap_or_default();
        if !parsed.is_empty() {
            out.push_str(&format!("tags: {}\n", parsed.join(", ")));
        }
    }
    out.push_str(&mem.content);
    out.push('\n');
    if let Some(facts) = &mem.facts {
        let parsed: Vec<String> = serde_json::from_str(facts).unwrap_or_default();
        if !parsed.is_empty() {
            out.push_str(&format!("facts: {}\n", parsed.join("; ")));
        }
    }
    out.push_str("---\n");
    out
}

fn format_compact(
    results: &[retrieve::CompactResult],
    omitted: usize,
    omitted_file: Option<&std::path::Path>,
) -> String {
    let total = results.len() + omitted;
    let mut header = format!("--- Memory Context ({} results", total);
    if omitted > 0 {
        if let Some(path) = omitted_file {
            header.push_str(&format!(
                " — {} included in full in {}",
                omitted,
                path.display()
            ));
        }
    }
    header.push_str(") ---\n");
    let mut out = header;
    for r in results {
        let date = r.created_at.get(..10).unwrap_or(&r.created_at);
        out.push_str(&format!(
            "[{:<18} {:.2}] {}  {}  ~{}c  {}\n",
            r.memory_type, r.importance, r.id, date, r.content_len, r.title
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
