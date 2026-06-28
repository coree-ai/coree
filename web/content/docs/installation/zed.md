+++
title = "Zed"
description = "Install coree as a Zed context server extension."
weight = 65
template = "page.html"
+++

coree is available as a Zed extension that registers it as a context server (MCP server) in the Zed AI assistant.

## Install via Extension Marketplace

1. Open Zed and press `Ctrl+Shift+X` (Linux/Windows) or `Cmd+Shift+X` (macOS) to open the Extensions panel.
2. Search for **Coree** and click **Install**.
3. Restart Zed.

The extension starts the coree context server automatically when you open the AI panel.

## Manual config

To configure coree without the extension, add to your Zed settings (`~/.config/zed/settings.json` on Linux, `~/Library/Application Support/Zed/settings.json` on macOS):

```json
{
  "context_servers": {
    "coree": {
      "command": {
        "path": "npx",
        "args": ["--yes", "@coree-ai/coree@0.16.0", "serve"],
        "env": {}
      }
    }
  }
}
```

For project-scoped config, use `.zed/settings.json` at your project root.

Note: Zed uses `"context_servers"` (not `"mcpServers"`).

## Context file

Copy `CLAUDE.md` to your project root so the AI assistant loads coree usage instructions:

```bash
curl -fsSL https://raw.githubusercontent.com/coree-ai/zed/main/CLAUDE.md -o CLAUDE.md
```

## Hooks

Zed does not expose lifecycle hook events for context server extensions. Context injection is driven by the agent following the instructions in `CLAUDE.md`.

## Environment variables

Set these in your shell profile so Zed inherits them:

```bash
export COREE__MEMORY__REMOTE_AUTH_TOKEN=your-token
export COREE__MEMORY__REMOTE_URL=libsql://your-db.turso.io
```

## Verify

Open the Zed AI panel and ask:

```
call the diagnose tool and show me the output
```
