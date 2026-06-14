+++
title = "Cursor"
description = "Install coree as a Cursor MCP server."
weight = 40
template = "page.html"
+++

Cursor supports MCP servers via a JSON config file. coree can be installed at project scope or user scope.

## Install

### Project scope

Create or edit `.cursor/mcp.json` in your project root:

```json
{
  "mcpServers": {
    "coree": {
      "command": "npx",
      "args": ["--yes", "@coree-ai/coree@0.15.0", "serve"]
    }
  }
}
```

### User scope

Create or edit `~/.cursor/mcp.json`:

```json
{
  "mcpServers": {
    "coree": {
      "command": "npx",
      "args": ["--yes", "@coree-ai/coree@0.15.0", "serve"]
    }
  }
}
```

User scope installs coree for all projects. Project scope installs it only for the current project.

## Context file

Copy `.cursorrules` to your project root so Cursor's agent loads coree usage instructions:

```bash
curl -fsSL https://raw.githubusercontent.com/coree-ai/coree/main/integrations/cursor/.cursorrules \
  -o .cursorrules
```

Cursor reads `.cursorrules` from the project root and includes it as system context for agent sessions. It covers the primary `search()` entry point, memory hygiene guidelines, and tool descriptions.

The MCP config is also available as a downloadable file:

```bash
curl -fsSL https://raw.githubusercontent.com/coree-ai/coree/main/integrations/cursor/mcp.json \
  -o .cursor/mcp.json
```

## Hooks

Cursor does not support lifecycle hooks. Context injection is driven by the agent following the instructions in `.cursorrules`.

## Enable in Cursor

After adding the config, restart Cursor or reload the window. The coree MCP server will appear in Cursor's MCP panel (Settings > MCP). Enable it if it is not already active.

## Verify

Open a Cursor Agent session and ask:

```
call the diagnose tool and show me the output
```

To remove, delete the `coree` entry from your `.cursor/mcp.json` or `~/.cursor/mcp.json` file manually.
