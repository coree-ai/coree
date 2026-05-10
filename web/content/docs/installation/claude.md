+++
title = "Claude Code"
description = "Install coree as a Claude Code marketplace plugin."
weight = 10
template = "page.html"
+++

coree is distributed through the Claude Code plugin marketplace and installs with two commands.

## Install

```bash
claude plugin marketplace add github:coree-ai/coree
claude plugin install coree
```

The first command registers the marketplace source from GitHub. The second installs the plugin from that source into Claude Code's plugin cache.

On first use, Claude Code invokes coree via `npx`, which downloads the package to `~/.npm/_npx/` and caches it. Subsequent invocations use the cache and start quickly.

## What gets installed

The plugin installs three config files into Claude Code's plugin cache:

- **`.mcp.json`** - registers the coree MCP server
- **`hooks.json`** - wires `session_context()` to the `UserPromptSubmit` and `SessionStart` hooks
- **`plugin.json`** - plugin metadata and version pin

No binary is installed at plugin-install time. The binary is fetched via `npx` on first use.

## Context file

A `CLAUDE.md` file is placed in your project root (or appended if one exists) with instructions for the agent: when to call `search()`, how to store memories, and what the tools do. You can edit this file freely.

## Updating

```bash
claude plugin update coree
```

Claude Code checks the GitHub marketplace source for a new version and copies updated config files if the version has changed.

## Uninstall

```bash
claude plugin uninstall coree
```

This removes the plugin config files. The npx cache at `~/.npm/_npx/` is unaffected - clear it manually if needed.

## Verify

After installation, start a session and run:

```
call the diagnose tool
```

The `diagnose` MCP tool reports server state, database status, and any initialisation errors.
