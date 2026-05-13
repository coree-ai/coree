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
| `BeforeAgent` | `inject --type prompt --budget 8000` | Injects relevant memories before each agent turn (up to 8 000 tokens) |

These run `npx --yes @coree-ai/coree@<version> inject ...` and prepend the output to the prompt. No manual configuration is required.

## Context file

`GEMINI.md` is the Gemini equivalent of `CLAUDE.md`. It is automatically used by Gemini CLI as context for the session. It covers the primary `search()` entry point, memory hygiene guidelines, and tool descriptions.

## Environment Variables & Redaction

Gemini CLI **redacts sensitive environment variables** (like `TOKEN`, `AUTH`, or `SECRET`) by default when passing them to extensions.

To use remote storage, you must have your `COREE__MEMORY__REMOTE_AUTH_TOKEN` available in your shell environment (e.g., via `export` or a `.env` file). The coree extension is pre-configured to allow these specific variables to pass through the redaction filter.

### Configuration via settings.json

Alternatively, you can explicitly configure the environment in `~/.gemini/settings.json`:

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
