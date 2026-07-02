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

To configure coree without the extension, add to your Zed settings file:

- Linux: `~/.config/zed/settings.json`
- macOS: `~/Library/Application Support/Zed/settings.json`
- Windows: `%APPDATA%\Zed\settings.json`

```json
{
  "context_servers": {
    "coree": {
      "command": {
        "path": "npx",
        "args": ["--yes", "@coree-ai/coree@0.17.0", "serve"],
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
# Linux / macOS
curl -fsSL https://raw.githubusercontent.com/coree-ai/zed/main/CLAUDE.md -o CLAUDE.md
```

```powershell
# Windows (PowerShell)
Invoke-WebRequest -Uri "https://raw.githubusercontent.com/coree-ai/zed/main/CLAUDE.md" -OutFile CLAUDE.md
```

## Hooks

Zed does not expose lifecycle hook events for context server extensions. Context injection is driven by the agent following the instructions in `CLAUDE.md`.

## Environment variables

Set these in your shell profile so Zed inherits them:

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

Zed extensions auto-update by default. If you have disabled auto-updates for the Coree extension, update it manually from the Extensions panel (`Ctrl+Shift+X` / `Cmd+Shift+X`).

To update the coree binary version used by a manual config, change the version in the `args` array in your `settings.json`:

```json
"args": ["--yes", "@coree-ai/coree@0.17.0", "serve"]
```

The npx cache at `~/.npm/_npx/` (`%LocalAppData%\npm-cache\_npx\` on Windows) is reused automatically.

## Verify

Open the Zed AI panel and ask:

```
call the diagnose tool and show me the output
```

## Troubleshooting

**Context server does not appear in the AI panel:**
Restart Zed after installing the extension or editing `settings.json`. Check that the `context_servers.coree` entry is in the correct settings file (see paths above).

**Server starts but times out on first use:**
First-run downloads the platform binary and embedding model, which can take 30-90 seconds. Wait for the download to complete - subsequent starts are fast.

**Search returns results from the wrong project:**
Set `COREE__PROJECT_ROOT` in the `env` block of your `context_servers.coree` config so coree knows which project directory to use.

**npx hangs or fails:**
Ensure Node.js 18+ is installed. Check that your network allows npm registry access (`registry.npmjs.org`).
