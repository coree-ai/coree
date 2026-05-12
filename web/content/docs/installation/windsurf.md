+++
title = "Windsurf"
description = "Install coree as a Windsurf MCP server with lifecycle hooks."
weight = 70
template = "page.html"
+++

Windsurf supports MCP servers via a global config file and workspace-level lifecycle hooks.

## Install

### Step 1 - MCP server

Merge the coree server entry into `~/.codeium/windsurf/mcp_config.json`:

```json
{
  "mcpServers": {
    "coree": {
      "command": "npx",
      "args": ["--yes", "@coree-ai/coree@0.13.0", "serve"],
      "env": {
        "COREE__MEMORY__REMOTE_AUTH_TOKEN": "${COREE__MEMORY__REMOTE_AUTH_TOKEN}",
        "COREE__MEMORY__REMOTE_URL": "${COREE__MEMORY__REMOTE_URL}"
      }
    }
  }
}
```

Restart Windsurf after saving.

### Step 2 - Hooks (optional)

Windsurf supports lifecycle hooks that inject coree context automatically. Copy the workspace hooks template to your project root:

```bash
mkdir -p .windsurf
curl -fsSL https://raw.githubusercontent.com/coree-ai/windsurf/main/.windsurf/hooks.json \
  -o .windsurf/hooks.json
```

This wires two hooks:

- **`pre_user_prompt`** - injects relevant memories before each prompt (`show_output: true` so you can see what was injected)
- **`post_cascade_response`** - saves the session summary after each response

For user-scope hooks that apply to all projects, copy to `~/.codeium/windsurf/hooks.json` instead. Both files are merged at startup (system → user → workspace).

## Verify

Open a Windsurf Cascade session and ask:

```
call the diagnose tool and show me the output
```

## Notes

Windsurf's MCP Marketplace is curated and not self-serve. The manual config above is the only supported installation path. Hooks are user-configured only - there is no distributable plugin format that installs them automatically.
