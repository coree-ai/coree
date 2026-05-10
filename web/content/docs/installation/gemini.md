+++
title = "Gemini CLI"
description = "Install coree as a Gemini CLI extension."
weight = 20
template = "page.html"
+++

coree is available as a Gemini CLI extension.

## Install

```bash
gemini extension install github:coree-ai/coree
```

This installs the extension from the GitHub repository. The extension config at `agents/gemini/gemini-extension.json` registers the coree MCP server and links the `GEMINI.md` context file.

## What gets installed

The extension provides:

- **MCP server registration** - coree runs via `npx --yes @coree-ai/coree@{{ version }} serve`
- **`GEMINI.md` context** - instruction file placed in your project, tells the agent how to use the memory and search tools
- **Settings entry** - optional `COREE__MEMORY__REMOTE_AUTH_TOKEN` for remote sync

## Context file

`GEMINI.md` is the Gemini equivalent of `CLAUDE.md`. It is automatically used by Gemini CLI as context for the session. It covers the primary `search()` entry point, memory hygiene guidelines, and tool descriptions.

## Remote sync (optional)

If you use Turso for remote storage, set the auth token via the extension settings or environment:

```bash
export COREE__MEMORY__REMOTE_AUTH_TOKEN=your-token
```

See [Configuration](/docs/configuration/) for the full remote sync setup.

## Verify

After installing, start a Gemini session in your project directory and run:

```
use the diagnose tool to check coree status
```
