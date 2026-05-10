+++
title = "OpenAI Codex"
description = "Install coree as a Codex plugin."
weight = 30
template = "page.html"
+++

coree supports OpenAI Codex via the Codex plugin system.

## Install

```bash
codex plugin add coree@coree
```

This installs the plugin from the npm registry. The plugin config registers coree as an MCP server running via `npx`.

## Sandbox filesystem access

Codex runs in a sandboxed environment with a read-only filesystem by default. coree needs write access to its data directory (`~/.local/share/coree/` on Linux) to operate.

Add a sandbox workspace write entry to your Codex config (`~/.codex/config.toml`):

```toml
[sandbox_workspace_write]
"/home/your-username/.local/share/coree" = true
```

Without this, coree starts in a degraded state. The `diagnose` tool will report a filesystem error if this is the cause.

## What gets installed

The plugin installs to `~/.codex/plugins/cache/coree/coree/<version>/`:

- **`.mcp.json`** - registers the MCP server: `npx --yes @coree-ai/coree@<version> serve`

The binary is fetched via npx on first use and cached in `~/.npm/_npx/`.

## Verify

Inside a Codex session:

```
call the diagnose mcp tool
```

If the filesystem sandbox is blocking writes, diagnose will report a `Read-only file system` error and show the remediation step.

## Notes

Codex's plugin sandbox is stricter than Claude Code's. If you see the MCP server start in degraded state, check:

1. The `sandbox_workspace_write` config entry points to the correct path
2. The npx cache is populated (run `npx --yes @coree-ai/coree@{{ version }} --version` outside the sandbox once to prime it)
