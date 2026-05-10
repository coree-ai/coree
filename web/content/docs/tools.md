+++
title = "MCP Tools"
description = "Reference for all MCP tools exposed by the coree server."
weight = 20
template = "page.html"
+++

The coree MCP server exposes the following tools. Tool names in the protocol are prefixed with the server name - e.g. `mcp__coree__search` in Claude Code.

---

## Search

### `search(query)`

**Primary entry point.** Searches memories and code simultaneously in a single ranked result set. Use this before reading files or starting a task.

```
search("rate limiting implementation")
search("why did we change the auth middleware")
search("Config::load signature")
```

Returns a combined list of memory snippets and code sections ranked by relevance.

### `search_code(query)`

Searches only the code index (source files and git history). Use when you specifically want code results without memory noise.

### `search_memory(query, limit?, detail?)`

Searches only stored memories. Use when you want to recall prior decisions or gotchas without code results.

---

## Memory management

### `store_memories(memories)`

Stores one or more memories. Each memory has:

| Field | Required | Description |
|-------|----------|-------------|
| `title` | yes | Short label, used in search results |
| `content` | yes | Full content of the memory |
| `type` | yes | `decision`, `gotcha`, `how-it-works`, `user`, `feedback`, `project`, `reference` |
| `importance` | no | 0.0-1.0, defaults to 0.5. Use >= 0.8 for critical items. |
| `tags` | no | Array of strings for filtering |
| `topic_key` | no | Stable key for upsert - if a memory with this key exists, it is updated |
| `facts` | no | Array of short fact strings extracted from the content |
| `source` | no | Provenance tag, e.g. `"reviewed"` |
| `pinned` | no | Boolean - pinned memories always appear in session context |

### `get_memories(ids)`

Fetches full content for one or more memory IDs. Use after `search()` returns IDs you want to read in full.

### `list_memories(type?, tags?, limit?, detail?)`

Lists stored memories, optionally filtered by type or tags.

### `delete_memories(ids)`

Permanently removes memories by ID.

### `pin_memories(ids, pin)`

Pins or unpins memories. Pinned memories always appear in session context regardless of relevance score.

### `list_stale_memories()`

Lists memories that have not been accessed recently. Useful for reviewing the memory store.

### `evict_stale_memories()`

Removes stale memories. Use with care - this is permanent.

---

## Session

### `session_context()`

Called at session start. Returns:
- Pending captures from the previous session (for review and promotion to memories)
- Most relevant memories for the current project
- Summary of recently active memory topics

This tool is called automatically by the hooks installed with the Claude Code and Gemini plugins.

### `capture_note(summary, context?)`

Stores a note for review at the next session start. Use for tentative observations during exploration - things you are not ready to commit as a full memory. Captures are reviewed by `session_context()` and can be promoted to memories or discarded.

---

## Code intelligence

### `get_symbol(name, file_path?)`

Looks up a symbol by name. Returns:
- Function/type signature
- File path and line range
- Git line-range history (which commits touched this code)
- Churn count (number of commits)
- Hotspot score (recent modification frequency - higher means more volatile)
- Cross-type similar results (functions/types with similar names)

`file_path` is optional but narrows the search when the symbol name is ambiguous.

---

## Remote

### `remote_sync()`

Triggers a manual sync between the local replica and the remote Turso database. Only relevant when `remote_mode = "replica"`.

---

## Agent integration

### `install(target, scope?)`

Installs coree for a supported agent target. Writes the appropriate config files.

| Target | Scope | Writes |
|--------|-------|--------|
| `cursor` | `project` | `.cursor/mcp.json` |
| `cursor` | `user` | `~/.cursor/mcp.json` |
| `claude` | - | Returns instructions (use marketplace instead) |
| `gemini` | - | Returns instructions (use extension instead) |

Scope is required for `cursor`.

### `uninstall(target, scope?)`

Reverses what `install` wrote.

---

## Diagnostics

### `diagnose()`

Reports the current state of the MCP server without requiring the database to be ready. Use when something is wrong.

Returns:
- Server state (Syncing / Ready / Failed)
- Database path and backend type
- Error from crash.log if available
- Pattern-matched failure reason and remediation steps
- Memory and index counts (if available)
