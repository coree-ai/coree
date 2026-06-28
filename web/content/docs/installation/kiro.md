+++
title = "Kiro"
description = "Install coree as a Kiro MCP server."
weight = 80
template = "page.html"
+++

Kiro supports MCP servers via a JSON config file at `.kiro/settings/mcp.json` (workspace) or `~/.kiro/settings/mcp.json` (global). Workspace settings take precedence.

## Install

### Workspace scope

```bash
mkdir -p .kiro/settings
curl -fsSL https://raw.githubusercontent.com/coree-ai/coree/main/integrations/kiro/mcp.json \
  -o .kiro/settings/mcp.json
```

### Global scope

```bash
mkdir -p ~/.kiro/settings
curl -fsSL https://raw.githubusercontent.com/coree-ai/coree/main/integrations/kiro/mcp.json \
  -o ~/.kiro/settings/mcp.json
```

### Via Kiro UI

Open **Settings > MCP Servers** and add:

- **Command**: `npx`
- **Args**: `--yes @coree-ai/coree@0.16.0 serve`

## Config

The workspace config pre-approves all coree tool calls so the agent does not prompt for confirmation on each memory read or write:

```json
{
  "mcpServers": {
    "coree": {
      "command": "npx",
      "args": ["--yes", "@coree-ai/coree@0.16.0", "serve"],
      "env": {
        "COREE__MEMORY__REMOTE_AUTH_TOKEN": "${COREE__MEMORY__REMOTE_AUTH_TOKEN}",
        "COREE__MEMORY__REMOTE_URL": "${COREE__MEMORY__REMOTE_URL}"
      },
      "disabled": false,
      "autoApprove": [
        "search", "search_code", "search_memory",
        "store_memories", "get_memories", "list_memories",
        "get_symbol", "session_context",
        "list_stale_memories", "evict_stale_memories",
        "pin_memories", "delete_memories"
      ],
      "disabledTools": []
    }
  }
}
```

`disabled: false` enables the server at startup. `autoApprove` lists all coree tool names so the agent can read and write memories without a confirmation prompt on each call.

## Hooks

Kiro does not support lifecycle hooks. Context injection is driven by the agent following coree's MCP tool instructions.

## Context file

Copy `.kiro/steering/coree.md` to your project so Kiro's agent loads coree usage instructions:

```bash
mkdir -p .kiro/steering
curl -fsSL https://raw.githubusercontent.com/coree-ai/coree/main/integrations/kiro/coree.md \
  -o .kiro/steering/coree.md
```

Kiro reads `.kiro/steering/coree.md` from the project root and includes it as system context for agent sessions. It covers the primary `search()` entry point, memory hygiene guidelines, and tool descriptions.

## Verify

Open a Kiro agent session and ask:

```
call the diagnose tool and show me the output
```
