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
curl -fsSL https://raw.githubusercontent.com/coree-ai/kiro/main/.kiro/settings/mcp.json \
  -o .kiro/settings/mcp.json
```

### Global scope

```bash
mkdir -p ~/.kiro/settings
curl -fsSL https://raw.githubusercontent.com/coree-ai/kiro/main/.kiro/settings/mcp.json \
  -o ~/.kiro/settings/mcp.json
```

### Via Kiro UI

Open **Settings > MCP Servers** and add:

- **Command**: `npx`
- **Args**: `--yes @coree-ai/coree@0.13.0 serve`

## Config

The workspace config pre-approves all coree tool calls so the agent does not prompt for confirmation on each memory read or write:

```json
{
  "mcpServers": {
    "coree": {
      "command": "npx",
      "args": ["--yes", "@coree-ai/coree@0.13.0", "serve"],
      "env": {
        "COREE__MEMORY__REMOTE_AUTH_TOKEN": "${COREE__MEMORY__REMOTE_AUTH_TOKEN}",
        "COREE__MEMORY__REMOTE_URL": "${COREE__MEMORY__REMOTE_URL}"
      },
      "disabled": false,
      "autoApprove": [
        "search", "search_code", "search_memory",
        "store_memories", "get_memories", "list_memories",
        "capture_note", "get_symbol", "session_context",
        "list_stale_memories", "evict_stale_memories",
        "pin_memories", "delete_memories"
      ],
      "disabledTools": []
    }
  }
}
```

## Verify

Open a Kiro agent session and ask:

```
call the diagnose tool and show me the output
```
