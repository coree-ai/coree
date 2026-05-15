+++
title = "VS Code"
description = "Install coree as an MCP server for VS Code agent features."
weight = 50
template = "page.html"
+++

VS Code supports MCP servers for agent features through GitHub Copilot and the agent panel. Configuration is via a JSON file.

## Project scope

Create `.vscode/mcp.json` in your project root:

```json
{
  "servers": {
    "coree": {
      "type": "stdio",
      "command": "npx",
      "args": ["--yes", "@coree-ai/coree@0.14.0", "serve"]
    }
  }
}
```

VS Code picks this up automatically when you open the project. The MCP server will appear in the agent panel.

Note: VS Code uses `"servers"` (not `"mcpServers"`) and requires `"type": "stdio"` on each entry.

## User scope

Add to your VS Code `settings.json` (`Ctrl+Shift+P` > "Open User Settings JSON"):

```json
{
  "mcp": {
    "servers": {
      "coree": {
        "type": "stdio",
        "command": "npx",
        "args": ["--yes", "@coree-ai/coree@0.14.0", "serve"]
      }
    }
  }
}
```

## Context file

Copy `.github/copilot-instructions.md` to your project so GitHub Copilot agent mode loads coree usage instructions:

```bash
mkdir -p .github
curl -fsSL https://raw.githubusercontent.com/coree-ai/vscode/main/.github/copilot-instructions.md \
  -o .github/copilot-instructions.md
```

GitHub Copilot reads `.github/copilot-instructions.md` and includes it as system context for agent sessions. It covers the primary `search()` entry point, memory hygiene guidelines, and tool descriptions.

## Hooks

VS Code does not support lifecycle hooks for MCP integrations. Context injection is driven by the agent following the instructions in `.github/copilot-instructions.md`.

## Enable

After adding the config, open the agent panel in VS Code. The coree server should appear in the MCP servers list. If it shows as disconnected, use "Restart MCP Server" from the command palette.

## Notes

VS Code's MCP support requires GitHub Copilot and a version of VS Code that includes agent mode. MCP server configuration format may change between VS Code releases - refer to the VS Code documentation for the current schema if the above does not work.

## Verify

In an agent session:

```
call the coree diagnose tool
```
