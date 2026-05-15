+++
title = "opencode"
description = "Add coree to opencode as an MCP server."
weight = 35
template = "page.html"
+++

opencode does not have a distributable MCP plugin format. Installation requires two manual steps.

## Install

### Step 1 - MCP server

Edit `~/.config/opencode/opencode.json` (global) or `opencode.json` in your project root (project-scoped), and add the coree server:

```json
{
  "mcp": {
    "coree": {
      "type": "local",
      "command": ["npx", "--yes", "@coree-ai/coree@0.14.0", "serve"],
      "environment": {
        "COREE__MEMORY__REMOTE_AUTH_TOKEN": "{env:COREE__MEMORY__REMOTE_AUTH_TOKEN}",
        "COREE__MEMORY__REMOTE_URL": "{env:COREE__MEMORY__REMOTE_URL}",
        "COREE__INDEX__REMOTE_AUTH_TOKEN": "{env:COREE__INDEX__REMOTE_AUTH_TOKEN}",
        "COREE__INDEX__REMOTE_URL": "{env:COREE__INDEX__REMOTE_URL}"
      },
      "enabled": true,
      "timeout": 120000
    }
  }
}
```

Notes on the format:
- `command` is an array combining the executable and arguments (unlike the separate `command`/`args` used by other tools)
- `{env:VAR}` forwards the named variable from your shell environment; unset variables are passed as empty strings
- `timeout` is in milliseconds (120 000 ms = 2 minutes, needed for first-run model download)

### Step 2 - Context file

Copy `opencode.md` to your project root so the agent loads coree usage instructions:

```bash
curl -fsSL https://raw.githubusercontent.com/coree-ai/opencode/main/opencode.md \
  -o opencode.md
```

opencode loads `opencode.md` from the project root as agent instructions. For a global context file that applies to all projects, add it to `~/.config/opencode/AGENTS.md` instead.

## Hooks

opencode does not expose lifecycle hooks for MCP integrations. Context injection is driven by the agent following the instructions in `opencode.md`.

## Verify

After configuration, start an opencode session and run:

```
call the diagnose tool
```

The `diagnose` MCP tool reports server state, database status, and any initialisation errors.
