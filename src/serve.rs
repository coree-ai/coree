use anyhow::Result;
use rmcp::{
    ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{Implementation, InitializeResult, ServerCapabilities},
    tool, tool_handler, tool_router,
    transport::stdio,
};
use schemars::JsonSchema;
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::{
    config::Config,
    db::Db,
    embed::Embedder,
    migrations,
    project_id,
    retrieve,
    store::{self, WriteLock},
};

#[derive(Clone)]
struct MemsoServer {
    conn: Arc<libsql::Connection>,
    embedder: Arc<Mutex<Embedder>>,
    write_lock: WriteLock,
    session_id: String,
    project_id: String,
    tool_router: ToolRouter<Self>,
}

// --- Tool input schemas ---

#[derive(Debug, Deserialize, JsonSchema)]
struct StoreMemoryInput {
    /// Full text of the memory to store.
    content: String,
    /// Memory type: decision | gotcha | problem-solution | how-it-works |
    /// what-changed | trade-off | preference | discovery | workflow | fact
    #[serde(rename = "type")]
    memory_type: String,
    /// Short summary shown in search results (one line).
    title: String,
    /// Stable slug for upsert semantics, e.g. "auth-session-store". Omit to always append.
    #[serde(default)]
    topic_key: Option<String>,
    /// Array of short discrete facts extracted from the content.
    #[serde(default)]
    facts: Vec<String>,
    /// Array of tag strings.
    #[serde(default)]
    tags: Vec<String>,
    /// Importance 0.0-1.0. Use 0.9+ for architecture decisions, 0.7+ for gotchas.
    #[serde(default = "default_importance")]
    importance: f32,
    /// Project scope. Omit to use the server's configured project_id.
    #[serde(default)]
    project_id: Option<String>,
}

fn default_importance() -> f32 { 0.5 }

#[derive(Debug, Deserialize, JsonSchema)]
struct SearchMemoryInput {
    /// Natural language search query.
    query: String,
    /// Project scope. Omit to use the server's configured project_id.
    #[serde(default)]
    project_id: Option<String>,
    /// Maximum results to return (default 5).
    #[serde(default = "default_search_limit")]
    limit: usize,
}

fn default_search_limit() -> usize { 5 }

#[derive(Debug, Deserialize, JsonSchema)]
struct GetMemoryInput {
    /// ID of the memory to fetch in full.
    id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ListMemoriesInput {
    /// Project scope. Omit to use the server's configured project_id.
    #[serde(default)]
    project_id: Option<String>,
    /// Filter by type (optional).
    #[serde(default, rename = "type")]
    memory_type: Option<String>,
    /// Filter by tags (optional).
    #[serde(default)]
    tags: Vec<String>,
    /// Maximum results to return (default 20).
    #[serde(default = "default_list_limit")]
    limit: usize,
}

fn default_list_limit() -> usize { 20 }

#[derive(Debug, Deserialize, JsonSchema)]
struct DeleteMemoryInput {
    /// ID of the memory to delete.
    id: String,
}

// --- Tool implementations ---

#[tool_router]
impl MemsoServer {
    #[tool(description = "Store or upsert a memory. Use topic_key for upsert semantics.")]
    async fn store_memory(&self, Parameters(input): Parameters<StoreMemoryInput>) -> Result<String, String> {
        let project = input.project_id.unwrap_or_else(|| self.project_id.clone());
        let req = store::StoreRequest {
            content: input.content,
            memory_type: input.memory_type,
            title: input.title,
            tags: input.tags,
            topic_key: input.topic_key,
            project_id: project,
            session_id: self.session_id.clone(),
            importance: input.importance,
            facts: input.facts,
        };
        let mut embedder = self.embedder.lock().await;
        store::store_memory(&self.conn, &mut embedder, &self.write_lock, req, 30)
            .await
            .map(|r| if r.upserted { format!("Updated memory {}", r.id) } else { format!("Stored memory {}", r.id) })
            .map_err(|e| format!("store_memory failed: {e}"))
    }

    #[tool(description = "Search memories using semantic + keyword search. Returns compact summaries with IDs. Use get_memory to fetch full content.")]
    async fn search_memory(&self, Parameters(input): Parameters<SearchMemoryInput>) -> Result<String, String> {
        let project = input.project_id.unwrap_or_else(|| self.project_id.clone());
        let mut embedder = self.embedder.lock().await;
        retrieve::search(&self.conn, &mut embedder, &input.query, &project, input.limit)
            .await
            .map(|results| if results.is_empty() { "No memories found.".to_string() } else { format_compact(&results) })
            .map_err(|e| format!("search_memory failed: {e}"))
    }

    #[tool(description = "Fetch the full content of a specific memory by ID.")]
    async fn get_memory(&self, Parameters(input): Parameters<GetMemoryInput>) -> Result<String, String> {
        retrieve::get_full(&self.conn, &input.id)
            .await
            .map_err(|e| format!("get_memory failed: {e}"))
            .and_then(|opt| {
                opt.map(|m| {
                    let facts: Vec<String> = m.facts.as_deref()
                        .and_then(|f| serde_json::from_str(f).ok()).unwrap_or_default();
                    let tags: Vec<String> = m.tags.as_deref()
                        .and_then(|t| serde_json::from_str(t).ok()).unwrap_or_default();
                    let facts_str = if facts.is_empty() { "none".to_string() }
                        else { format!("- {}", facts.join("\n- ")) };
                    format!(
                        "[{memory_type}] {title}\nID: {id}\nCreated: {created}\nImportance: {imp:.1}\nTags: {tags}\n\nContent:\n{content}\n\nFacts:\n{facts}",
                        memory_type = m.memory_type, title = m.title, id = m.id,
                        created = m.created_at, imp = m.importance,
                        tags = tags.join(", "), content = m.content, facts = facts_str,
                    )
                })
                .ok_or_else(|| format!("Memory {} not found", input.id))
            })
    }

    #[tool(description = "List memories with optional filters. Returns compact summaries.")]
    async fn list_memories(&self, Parameters(input): Parameters<ListMemoriesInput>) -> Result<String, String> {
        let project = input.project_id.unwrap_or_else(|| self.project_id.clone());
        retrieve::list(&self.conn, &project, input.memory_type.as_deref(), &input.tags, input.limit)
            .await
            .map(|results| if results.is_empty() { "No memories found.".to_string() } else { format_compact(&results) })
            .map_err(|e| format!("list_memories failed: {e}"))
    }

    #[tool(description = "Delete a memory by ID.")]
    async fn delete_memory(&self, Parameters(input): Parameters<DeleteMemoryInput>) -> Result<String, String> {
        self.conn
            .execute("UPDATE memories SET status = 'deleted' WHERE id = ?1", libsql::params![input.id.clone()])
            .await
            .map_err(|e| format!("delete_memory failed: {e}"))
            .and_then(|rows| {
                if rows > 0 { Ok(format!("Deleted memory {}", input.id)) }
                else { Err(format!("Memory {} not found", input.id)) }
            })
    }
}

#[tool_handler]
impl ServerHandler for MemsoServer {
    fn get_info(&self) -> InitializeResult {
        InitializeResult::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("memso", env!("CARGO_PKG_VERSION")))
            .with_instructions(
                "Persistent memory across sessions. \
                 Use store_memory to save decisions, gotchas, preferences and discoveries. \
                 Use search_memory at session start and before significant tasks.",
            )
    }
}

pub async fn run(config: Config) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let pid = project_id::resolve(&cwd, config.memory.project_id.as_deref());

    eprintln!("memso: opening database...");
    let db = Db::open(&config).await?;
    let conn = Arc::new(db.conn);

    eprintln!("memso: running migrations...");
    migrations::run(&conn).await?;

    eprintln!("memso: loading embedding model...");
    let embedder = Arc::new(Mutex::new(Embedder::load()?));

    let session_id = Uuid::new_v4().to_string();
    eprintln!("memso: session {session_id}, project \"{pid}\"");
    eprintln!("memso: ready");

    let server = MemsoServer {
        conn,
        embedder,
        write_lock: store::new_write_lock(),
        session_id,
        project_id: pid,
        tool_router: MemsoServer::tool_router(),
    };

    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}

fn format_compact(results: &[retrieve::CompactResult]) -> String {
    let mut out = format!("--- Memory Context ({} results) ---\n", results.len());
    for r in results {
        let date = r.created_at.get(..10).unwrap_or(&r.created_at);
        out.push_str(&format!("[{:<18}] {}  {}  {}\n", r.memory_type, r.id, date, r.title));
    }
    out.push_str("---");
    out
}
