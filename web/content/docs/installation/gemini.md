+++
title = "Gemini CLI"
description = "Install coree as a Gemini CLI extension."
weight = 20
template = "page.html"
+++

coree is available as a Gemini CLI extension.

## Install

```bash
gemini extension install github:coree-ai/gemini
```

This installs the extension from the `coree-ai/gemini` repository.

## What gets installed

The extension provides:

- **MCP server registration** - coree runs via `npx --yes @coree-ai/coree@<version> serve`
- **Hooks** - two lifecycle hooks installed automatically (see below)
- **`GEMINI.md` context** - instruction file placed in your project, tells the agent how to use the memory and search tools

## Hooks

Two hooks are installed automatically with the extension:

| Hook | Command | Purpose |
|------|---------|---------|
| `SessionStart` | `inject --type session` | Injects stale notes and session context at the start of each session |
| `BeforeAgent` | `inject --type prompt` | Injects relevant memories before each agent turn |

These run `npx --yes @coree-ai/coree@<version> inject ...` and prepend the output to the prompt. No manual configuration is required.

## Context file

`GEMINI.md` is the Gemini equivalent of `CLAUDE.md`. It is automatically used by Gemini CLI as context for the session. It covers the primary `search()` entry point, memory hygiene guidelines, and tool descriptions.

## Environment Variables & Redaction

Gemini CLI **redacts sensitive environment variables** (like `TOKEN`, `AUTH`, or `SECRET`) by default when passing them to extensions.

To use remote storage, you must have your `COREE__MEMORY__REMOTE_AUTH_TOKEN` available in your shell environment. The coree extension is pre-configured to allow these variables to pass through the redaction filter.

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

### Configuration via settings.json

Alternatively, you can explicitly configure the environment in your user settings file:

- Linux / macOS: `~/.gemini/settings.json`
- Windows: `%USERPROFILE%\.gemini\settings.json`

```json
{
  "mcpServers": {
    "coree": {
      "env": {
        "COREE__MEMORY__REMOTE_AUTH_TOKEN": "your-token-here"
      }
    }
  }
}
```

## Verify

After installing, start a Gemini session in your project directory and run:

```
use the diagnose tool to check coree status
```

## Updating

```bash
gemini extension update coree
```

Gemini CLI checks the GitHub source for a new version and updates the extension if the version has changed.

If `gemini extension update` is not available in your version, reinstall the extension:

```bash
gemini extension install github:coree-ai/gemini
```

## Troubleshooting

**Remote sync env vars are redacted:**
Gemini CLI redacts environment variables matching `TOKEN`, `AUTH`, or `SECRET` by default. The coree extension allows `COREE__MEMORY__REMOTE_AUTH_TOKEN` through, but if you have a custom variable name, add it to the extension's allowlist or set it via `settings.json` (see above).

**Server starts but times out on first use:**
First-run downloads the platform binary and embedding model, which can take 30-90 seconds. Wait for the download to complete - subsequent starts are fast.

**MCP server does not appear in the tool list:**
Restart Gemini CLI. Check that the extension was installed correctly with `gemini extension list`.

**npx hangs or fails:**
Ensure Node.js 18+ is installed. Check that your network allows npm registry access (`registry.npmjs.org`).
