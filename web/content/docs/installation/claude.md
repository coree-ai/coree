+++
title = "Claude Code"
description = "Install coree as a Claude Code marketplace plugin."
weight = 10
template = "page.html"
+++

coree is distributed through the Claude Code plugin marketplace and installs with two commands.

## Install

```bash
claude plugin marketplace add github:coree-ai/claude
claude plugin install coree
```

The first command registers the marketplace source from GitHub. The second installs the plugin from that source into Claude Code's plugin cache.

On first use, Claude Code invokes coree via `npx`, which downloads the package to `~/.npm/_npx/` and caches it. Subsequent invocations use the cache and start quickly.

## What gets installed

The plugin installs four config files into Claude Code's plugin cache:

- **`.mcp.json`** - registers the coree MCP server
- **`hooks.json`** - wires four lifecycle hooks (see below)
- **`plugin.json`** - plugin metadata and version pin

No binary is installed at plugin-install time. The binary is fetched via `npx` on first use.

## Hooks

Four hooks are installed automatically and fire without any manual configuration:

| Hook | Command | Purpose |
|------|---------|---------|
| `SessionStart` | `inject --type session` | Injects stale notes and session context at the start of each session |
| `UserPromptSubmit` | `inject --type prompt` | Injects relevant memories before each user prompt |
| `Stop` | `inject --type stop` | Runs a post-session memory save when the agent stops |
| `PostCompact` | `inject --type compact` | Re-injects context after Claude compacts the conversation |

These hooks are the primary mechanism for automatic context injection. They run `npx --yes @coree-ai/coree@<version> inject ...` and prepend the output to the prompt or system message.

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

## Troubleshooting

**`claude plugin update coree` reports "already at latest version" after a new release:**
Claude Code maintains a private npm cache at `~/.claude/plugins/npm-cache/` (Linux/macOS) or `%USERPROFILE%\.claude\plugins\npm-cache\` (Windows) that is not automatically invalidated. Clear the cache and reinstall:

```bash
# Linux / macOS
rm -rf ~/.claude/plugins/npm-cache
rm -rf ~/.claude/plugins/cache/coree

# Windows (PowerShell)
Remove-Item -Recurse -Force "$env:USERPROFILE\.claude\plugins\npm-cache"
Remove-Item -Recurse -Force "$env:USERPROFILE\.claude\plugins\cache\coree"
```

Then run `claude plugin update coree` again. This is a known Claude Code bug (issues #37670, #33253).

**Server starts but times out on first use:**
First-run downloads the platform binary and embedding model, which can take 30-90 seconds. Wait for the download to complete - subsequent starts are fast.

**Hooks do not fire:**
Check that the plugin config files are present in the plugin cache. Run `claude plugin list` to verify the plugin is installed and enabled.

**npx hangs or fails:**
Ensure Node.js 18+ is installed. Check that your network allows npm registry access (`registry.npmjs.org`).
