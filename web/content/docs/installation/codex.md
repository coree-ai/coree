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

The plugin installs to `~/.codex/plugins/cache/coree/coree/<version>/`:

- **`.mcp.json`** - registers the MCP server: `npx --yes @coree-ai/coree@<version> serve`
- **`hooks/hooks.json`** - wires lifecycle hooks for automatic context injection
- **`AGENTS.md`** - optional agent usage instructions
- **`.codex-plugin/plugin.json`** - plugin metadata and version pin

The binary is fetched via npx on first use and cached in `~/.npm/_npx/`.

## Environment variables

If you use coree's remote sync, the following env vars must be set in your shell so Codex forwards them to the MCP process:

- `COREE__MEMORY__REMOTE_AUTH_TOKEN`
- `COREE__MEMORY__REMOTE_URL`

The plugin's `.mcp.json` already lists these in `env_vars` so Codex will forward them if they are present in your shell environment.

## Verify

After installation, start a session and run:

```
call the diagnose mcp tool
```

Diagnose reports the current server state and any configuration issues.
