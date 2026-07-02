+++
title = "OpenClaw"
description = "Install coree in OpenClaw via the codex-compatible plugin format."
weight = 75
template = "page.html"
+++

OpenClaw reads the Codex plugin format natively, so the coree plugin can be installed directly from the `coree-ai/openclaw` repository.

## Install via plugin command

```bash
openclaw plugins install git:github.com/coree-ai/openclaw
```

This installs the `.codex-plugin/plugin.json` manifest and registers coree as an MCP server.

## Manual config

OpenClaw uses `mcp.servers` (not `mcpServers`). Add to your project's `openclaw.json` or your global config:

- Linux / macOS: `~/.openclaw/config.json`
- Windows: `%USERPROFILE%\.openclaw\config.json`

```json
{
  "mcp": {
    "servers": {
      "coree": {
        "command": "npx",
        "args": ["--yes", "@coree-ai/coree@0.17.0", "serve"],
        "env": {
          "COREE__MEMORY__REMOTE_AUTH_TOKEN": "${COREE__MEMORY__REMOTE_AUTH_TOKEN}",
          "COREE__MEMORY__REMOTE_URL": "${COREE__MEMORY__REMOTE_URL}"
        }
      }
    }
  }
}
```

## Context file

Copy `AGENTS.md` to your project root so the agent loads coree usage instructions:

```bash
# Linux / macOS
curl -fsSL https://raw.githubusercontent.com/coree-ai/openclaw/main/AGENTS.md -o AGENTS.md
```

```powershell
# Windows (PowerShell)
Invoke-WebRequest -Uri "https://raw.githubusercontent.com/coree-ai/openclaw/main/AGENTS.md" -OutFile AGENTS.md
```

## Hooks

OpenClaw lifecycle hooks (`llm_input`, `llm_output`, etc.) require TypeScript/JavaScript plugin code and cannot be declared as shell command configs. Context injection is driven by the agent following the instructions in `AGENTS.md`.

## Codex compatibility

If you use both OpenClaw and Codex CLI, the `coree-ai/codex` repo works in both tools. You can use that single repo instead.

## Verify

Start an OpenClaw session and ask:

```
call the diagnose tool and show me the output
```

## Updating

```bash
openclaw plugins install git:github.com/coree-ai/openclaw
```

Re-running the install command updates the plugin if the repository has a newer version.

## Troubleshooting

**MCP server does not appear in the tool list:**
Restart OpenClaw. Check that the `mcp.servers.coree` entry is in your `openclaw.json` or global config file (see paths above).

**Server starts but times out on first use:**
First-run downloads the platform binary and embedding model, which can take 30-90 seconds. Wait for the download to complete - subsequent starts are fast.

**Context injection not happening:**
OpenClaw lifecycle hooks require TypeScript/JavaScript plugin code. Context injection is driven by the agent following `AGENTS.md` - make sure the file is in your project root.

**npx hangs or fails:**
Ensure Node.js 18+ is installed. Check that your network allows npm registry access (`registry.npmjs.org`).
