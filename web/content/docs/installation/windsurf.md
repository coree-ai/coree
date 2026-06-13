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
      "args": ["--yes", "@coree-ai/coree@0.14.1", "serve"],
      "env": {
        "COREE__MEMORY__REMOTE_AUTH_TOKEN": "${COREE__MEMORY__REMOTE_AUTH_TOKEN}",
        "COREE__MEMORY__REMOTE_URL": "${COREE__MEMORY__REMOTE_URL}"
      }
    }
  }
}
```

Restart Windsurf after saving.

### Step 2 - Hooks (optional but recommended)

Windsurf supports lifecycle hooks that inject coree context automatically. Copy the workspace hooks template to your project root:

```bash
mkdir -p .windsurf
curl -fsSL https://raw.githubusercontent.com/coree-ai/coree/main/integrations/windsurf/hooks.json \
  -o .windsurf/hooks.json
```

For user-scope hooks that apply to all projects, copy to `~/.codeium/windsurf/hooks.json` instead. Hook configs from both scopes are merged at startup (system → user → workspace).

## Hooks

Two hooks are included in the workspace template. They are not installed automatically - copy the file as described above:

| Hook | Command | Purpose |
|------|---------|---------|
| `pre_user_prompt` | `inject --type prompt` | Injects relevant memories before each Cascade prompt. Output is shown in the Cascade panel (`show_output: true`). |
| `post_cascade_response` | `inject --type stop` | Runs post-turn processing after each Cascade response. |

The full hook config:

```json
{
  "hooks": {
    "pre_user_prompt": [
      {
        "command": "npx --yes @coree-ai/coree@0.14.1 inject --type prompt",
        "show_output": true
      }
    ],
    "post_cascade_response": [
      {
        "command": "npx --yes @coree-ai/coree@0.14.1 inject --type stop"
      }
    ]
  }
}
```

Windsurf exposes 12 total hook events. Pre-hooks can block execution by exiting with code 2. The full list: `pre_user_prompt`, `pre_read_code`, `pre_write_code`, `pre_run_command`, `pre_mcp_tool_use`, `post_cascade_response`, `post_cascade_response_with_transcript`, `post_read_code`, `post_write_code`, `post_run_command`, `post_mcp_tool_use`, `post_setup_worktree`.

## Context file

Copy `.windsurfrules` to your project so Windsurf Cascade loads coree usage instructions:

```bash
curl -fsSL https://raw.githubusercontent.com/coree-ai/coree/main/integrations/windsurf/windsurfrules \
  -o .windsurfrules
```

Windsurf reads `.windsurfrules` from the project root and includes it as system context for Cascade sessions. It covers the primary `search()` entry point, memory hygiene guidelines, and tool descriptions.

The MCP config is also available as a downloadable file:

```bash
curl -fsSL https://raw.githubusercontent.com/coree-ai/coree/main/integrations/windsurf/mcp_config.json \
  -o ~/.codeium/windsurf/mcp_config.json
```

## Verify

Open a Windsurf Cascade session and ask:

```
call the diagnose tool and show me the output
```

## Notes

Windsurf's MCP Marketplace is curated and not self-serve. The manual config above is the only supported installation path. Hooks are user-configured only - there is no distributable plugin format that installs them automatically.
