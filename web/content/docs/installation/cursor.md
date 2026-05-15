+++
title = "Cursor"
description = "Install coree as a Cursor MCP server."
weight = 40
template = "page.html"
+++

Cursor supports MCP servers via a JSON config file. coree can be installed at project scope or user scope.

## Install via CLI

If coree is already installed and on your PATH:

```bash
# project scope (writes .cursor/mcp.json)
coree install cursor --scope project

# user scope (writes ~/.cursor/mcp.json)
coree install cursor --scope user
```

## Manual install

### Project scope

Create or edit `.cursor/mcp.json` in your project root:

```json
{
  "mcpServers": {
    "coree": {
      "command": "npx",
      "args": ["--yes", "@coree-ai/coree@0.14.0", "serve"]
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
      "args": ["--yes", "@coree-ai/coree@0.14.0", "serve"]
    }
  }
}
```

User scope installs coree for all projects. Project scope installs it only for the current project.

## Context file

Copy `.cursorrules` to your project root so Cursor's agent loads coree usage instructions:

```bash
curl -fsSL https://raw.githubusercontent.com/coree-ai/cursor/main/.cursorrules \
  -o .cursorrules
```

Cursor reads `.cursorrules` from the project root and includes it as system context for agent sessions. It covers the primary `search()` entry point, memory hygiene guidelines, and tool descriptions.

## Hooks

Cursor does not support lifecycle hooks. Context injection is driven by the agent following the instructions in `.cursorrules`.

## Enable in Cursor

After adding the config, restart Cursor or reload the window. The coree MCP server will appear in Cursor's MCP panel (Settings > MCP). Enable it if it is not already active.

## Verify

Open a Cursor Agent session and ask:

```
call the diagnose tool and show me the output
```

## Uninstall

```bash
coree uninstall cursor --scope project
```

This removes the `coree` entry from `.cursor/mcp.json`. If the file becomes empty, it is left in place.
