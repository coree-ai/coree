+++
title = "Any MCP client"
description = "Add coree to any MCP-compatible agent."
weight = 60
template = "page.html"
+++

coree runs as a stdio MCP server. Any MCP-compatible agent or client can connect to it. This page is the authoritative generic setup guide - use it when no agent-specific installation page exists.

## How it works

The `npx` command downloads the `@coree-ai/coree` npm package on first use and selects the correct platform binary for your OS and architecture. The package's `optionalDependencies` declare four platform-specific packages (`coree-linux-x64`, `coree-linux-arm64`, `coree-darwin-arm64`, `coree-win32-x64`). npm installs only the one matching your system, so the correct binary is available immediately. No global install or manual binary download is required.

The first invocation downloads and caches the package in `~/.npm/_npx/` (Linux/macOS) or `%LocalAppData%\npm-cache\_npx\` (Windows). Subsequent invocations use the cache and start quickly (typically < 200 ms).

## MCP server config

```json
{
  "mcpServers": {
    "coree": {
      "command": "npx",
      "args": ["--yes", "@coree-ai/coree@0.16.0", "serve"]
    }
  }
}
```

Add this to whatever config format your MCP client uses. The server communicates over stdio using the standard MCP protocol.

If your client uses a unified `command` array (e.g. opencode):

```json
{
  "mcpServers": {
    "coree": {
      "command": ["npx", "--yes", "@coree-ai/coree@0.16.0", "serve"]
    }
  }
}
```

If you have the binary installed directly:

```
command: coree
args:    serve
```

### Timeout

Set a generous startup timeout. First-run downloads the platform binary and the embedding model, which can take 30-90 seconds:

```json
{
  "mcpServers": {
    "coree": {
      "command": "npx",
      "args": ["--yes", "@coree-ai/coree@0.16.0", "serve"],
      "timeout": 120000
    }
  }
}
```

Some clients auto-detect or use a default timeout. If the server fails to start on first use, check whether your client enforces a startup deadline.

## MCP Tools

Once connected, coree exposes these tools to the agent:

| Tool | Purpose |
|------|---------|
| `search(query)` | Primary entry point - searches memories and code simultaneously |
| `search_code(query)` | Code-only search |
| `search_memory(query)` | Memory-only search |
| `store_memories(memories)` | Store decisions, gotchas, discoveries |
| `get_memories(ids)` | Fetch full memory content by ID |
| `list_memories(type?, tags?)` | Browse stored memories |
| `pin_memories(ids, pin)` | Pin/unpin memories (pinned always surface) |
| `delete_memories(ids)` | Soft-delete memories |
| `get_symbol(name, file_path?)` | Look up functions/types with git history |
| `session_context()` | Load relevant memories at session start |
| `diagnose()` | Server state and remediation steps |
| `remote_sync()` | Manual sync to Turso (replica mode only) |

Full reference: [MCP Tools](/docs/tools).

## Context file

coree works best when the agent knows when to call its tools. For agents that load instruction files from the project root, add a `AGENTS.md` or `CLAUDE.md` file with coree usage guidance.

If your agent supports one of these files, copy the generic instructions:

```bash
# Linux / macOS
curl -fsSL https://raw.githubusercontent.com/coree-ai/coree/main/INSTRUCTIONS.md \
  -o AGENTS.md
```

```powershell
# Windows (PowerShell)
Invoke-WebRequest -Uri "https://raw.githubusercontent.com/coree-ai/coree/main/INSTRUCTIONS.md" -OutFile AGENTS.md
```

This file tells the agent to call `search()` before reading files, to store memories with `store_memories()`, and how to use each tool. Agents that auto-detect project-root context files will pick it up automatically.

## Environment variables

Any config field can be set via environment variable using `COREE__<SECTION>__<FIELD>` notation. Env vars take precedence over both project and global config files.

Common environment variables:

| Variable | Purpose |
|----------|---------|
| `COREE__PROJECT_ROOT` | Override project root directory (see below) |
| `COREE__PROJECT_ID` | Explicit project identifier |
| `COREE__MEMORY__MODE` | `managed` (default), `local`, `remote`, or `disabled` |
| `COREE__MEMORY__REMOTE_URL` | Turso/libSQL database URL |
| `COREE__MEMORY__REMOTE_AUTH_TOKEN` | Turso auth token |
| `COREE__INDEX__MODE` | `managed` (default), `local`, `remote`, or `disabled` |
| `COREE__INDEX__REMOTE_URL` | Turso database URL for code index |
| `COREE__INDEX__REMOTE_AUTH_TOKEN` | Turso auth token for code index |

### Remote sync example

```json
{
  "mcpServers": {
    "coree": {
      "command": "npx",
      "args": ["--yes", "@coree-ai/coree@0.16.0", "serve"],
      "env": {
        "COREE__MEMORY__MODE": "remote",
        "COREE__MEMORY__REMOTE_URL": "libsql://your-db.turso.io",
        "COREE__MEMORY__REMOTE_AUTH_TOKEN": "your-token"
      }
    }
  }
}
```

Full configuration reference: [Configuration](/docs/configuration).

## Project root

coree determines the project root by walking up from the current working directory, looking for `.coree.toml` or `.git`. This is how it knows which project a set of memories belongs to.

**If your MCP client spawns the server from a plugin directory, configuration directory, or other non-project path, coree will not find your project root.** Set `COREE__PROJECT_ROOT` explicitly:

```json
{
  "mcpServers": {
    "coree": {
      "command": "npx",
      "args": ["--yes", "@coree-ai/coree@0.16.0", "serve"],
      "env": {
        "COREE__PROJECT_ROOT": "/path/to/your/project"
      }
    }
  }
}
```

On Windows, use a backslash path:

```json
"env": {
  "COREE__PROJECT_ROOT": "C:\\Users\\you\\projects\\my-project"
}
```

When do you need this? Signs that the project root is wrong:
- `diagnose()` reports "no project" or a directory you do not recognize
- Memories from other projects appear in search results
- The server looks for `.coree.toml` in the wrong directory

## Verify

After adding the server config, restart your agent or reload the MCP panel. Then call the diagnostic tool:

```
call the diagnose tool
```

The `diagnose()` MCP tool reports:
- Server state (Syncing / Ready / Failed)
- Database path and backend type
- Memory and index counts (if available)
- Any initialisation errors with remediation steps

If the server is in "Syncing" state on first use, wait for the embedding model download to complete (30-90 seconds). The server transitions to "Ready" when the model is cached.

## Troubleshooting

**Server does not appear in agent's tool list:**
Check that the MCP config was saved to the correct file and path for your client. Restart the agent completely. Some agents require a window reload or settings refresh.

**Server starts but times out on first use:**
First-run downloads the platform binary and embedding model. Increase the startup timeout to 120000 ms (2 minutes) if your client supports it.

**Search returns results from the wrong project:**
Set `COREE__PROJECT_ROOT` (see above). The MCP client is likely spawning the server from a directory that is not your project root.

**npx hangs or fails:**
Ensure Node.js 18+ is installed. Check that your network allows npm registry access (`registry.npmjs.org`). Some corporate proxies block npm - set `NPM_CONFIG_REGISTRY` or configure `npx` proxy settings.

**Embedding model download stalls:**
coree downloads the embedding model on first startup from Hugging Face. If your network blocks Hugging Face, set `HF_HUB_ENABLE_HF_TRANSFER=0` and allow `huggingface.co:443`.

## Protocol version

coree implements MCP 2024-11-05. It uses stdio transport only. HTTP/SSE transport is not supported.

## Without MCP

If your agent does not support MCP at all, coree can still be used as a memory bank through its CLI interface:

```bash
# Linux / macOS
npx --yes @coree-ai/coree@0.16.0 search "deployment checklist"
npx --yes @coree-ai/coree@0.16.0 store --type decision --title "Architecture choice" --content "..."

# Run the server in single-turn mode for scripting
npx --yes @coree-ai/coree@0.16.0 serve --single-turn '{"method":"tools/call","params":{"name":"search","arguments":{"query":"rate limiting"}}}'
```

```powershell
# Windows (PowerShell)
npx --yes @coree-ai/coree@0.16.0 search "deployment checklist"
npx --yes @coree-ai/coree@0.16.0 store --type decision --title "Architecture choice" --content "..."
```
