+++
title = "OpenAI Codex"
description = "Install coree as a Codex plugin."
weight = 30
template = "page.html"
+++

coree supports OpenAI Codex via the Codex plugin system.

## Install

```bash
codex plugin marketplace add coree-ai/codex
codex plugin add coree@coree
```

The first command registers the marketplace source from GitHub. The second installs the plugin into Codex's plugin cache, which registers the coree MCP server and hooks.

## Hooks

The plugin bundles hooks automatically. Codex asks you to review and trust command hooks before they run; use `/hooks` after installing or updating the plugin.

| Hook | Purpose |
|------|---------|
| `SessionStart` | Injects stale notes and session context at the start of each session and after compaction |
| `UserPromptSubmit` | Injects relevant memories before each user prompt (up to 8 000 tokens) |
| `Stop` | Currently returns an empty JSON response while stop-continuation behavior is verified |

Without these hooks, coree still works as an MCP server - you can call tools manually. The hooks add automatic context injection equivalent to the Claude Code and Gemini CLI integrations.

## What gets installed

The plugin installs to the Codex plugin cache:

- Linux / macOS: `~/.codex/plugins/cache/coree/coree/<version>/`
- Windows: `%USERPROFILE%\.codex\plugins\cache\coree\coree\<version>\`

The directory contains:

- **`.mcp.json`** - registers the MCP server: `npx --yes @coree-ai/coree@<version> serve`
- **`hooks/hooks.json`** - wires lifecycle hooks for automatic context injection
- **`AGENTS.md`** - optional agent usage instructions
- **`.codex-plugin/plugin.json`** - plugin metadata and version pin

The binary is fetched via npx on first use and cached in `~/.npm/_npx/` (Linux/macOS) or `%LocalAppData%\npm-cache\_npx\` (Windows).

## Environment variables

If you use coree's remote sync, the following env vars must be set in your shell so Codex forwards them to the MCP process:

- `COREE__MEMORY__REMOTE_AUTH_TOKEN`
- `COREE__MEMORY__REMOTE_URL`

The plugin's `.mcp.json` already lists these in `env_vars` so Codex will forward them if they are present in your shell environment.

```bash
# Linux / macOS (bash/zsh)
export COREE__MEMORY__REMOTE_AUTH_TOKEN=your-token
export COREE__MEMORY__REMOTE_URL=libsql://your-db.turso.io
```

```powershell
# Windows (PowerShell)
$env:COREE__MEMORY__REMOTE_AUTH_TOKEN = "your-token"
$env:COREE__MEMORY__REMOTE_URL = "libsql://your-db.turso.io"
```

## Updating

```bash
codex plugin add coree@coree
```

Codex checks the marketplace source for a new version and updates the plugin cache if the version has changed. Use `/hooks` after updating to review and trust any new hook commands.

## Verify

After installation, start a session and run:

```
call the diagnose mcp tool
```

Diagnose reports the current server state and any configuration issues.

## Troubleshooting

**Plugin hooks do not fire:**
Run `/hooks` in Codex and verify the coree hooks are trusted. Codex asks you to review command hooks before they run - they will not fire until trusted.

**Server starts but times out on first use:**
First-run downloads the platform binary and embedding model, which can take 30-90 seconds. Wait for the download to complete - subsequent starts are fast.

**MCP server does not appear in the tool list:**
Restart Codex completely. Check that the plugin was installed to the correct cache directory (see paths above).

**npx hangs or fails:**
Ensure Node.js 18+ is installed. Check that your network allows npm registry access (`registry.npmjs.org`).
