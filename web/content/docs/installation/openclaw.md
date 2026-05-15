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

OpenClaw uses `mcp.servers` (not `mcpServers`). Add to your project's `openclaw.json` or `~/.openclaw/config.json`:

```json
{
  "mcp": {
    "servers": {
      "coree": {
        "command": "npx",
        "args": ["--yes", "@coree-ai/coree@0.14.1", "serve"],
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
curl -fsSL https://raw.githubusercontent.com/coree-ai/openclaw/main/AGENTS.md -o AGENTS.md
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
