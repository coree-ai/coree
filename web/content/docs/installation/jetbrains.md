+++
title = "JetBrains"
description = "Install coree as an MCP server for JetBrains AI Assistant."
weight = 85
template = "page.html"
+++

JetBrains AI Assistant supports MCP servers from IntelliJ IDEA 2025.1+ (AI Assistant plugin 251.26094.80.5+).

## Install via UI

1. Open your JetBrains IDE.
2. Go to **Settings | Tools | AI Assistant | Model Context Protocol (MCP)**.
3. Click **+** to add a new server.
4. Enter:
   - **Name**: `coree`
   - **Command**: `npx`
   - **Arguments**: `--yes @coree-ai/coree@0.17.0 serve`
5. Click **OK** and restart the IDE.

## Install via config file

Copy `mcp.json` to the IDE-version-specific config path:

| OS | Path |
|----|------|
| Linux | `~/.config/JetBrains/<IDE><version>/mcp.json` |
| macOS | `~/Library/Application Support/JetBrains/<IDE><version>/mcp.json` |
| Windows | `%APPDATA%\JetBrains\<IDE><version>\mcp.json` |

Where `<IDE>` is the product name (e.g. `IntelliJIdea`, `PyCharm`, `WebStorm`, `GoLand`) and `<version>` is the release year and major version (e.g. `2025.1`).

Example for IntelliJ IDEA 2025.1 on Linux:

```bash
curl -fsSL https://raw.githubusercontent.com/coree-ai/coree/main/integrations/jetbrains/mcp.json \
  -o ~/.config/JetBrains/IntelliJIdea2025.1/mcp.json
```

The config:

```json
{
  "mcpServers": {
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
```

## Context file

Copy `AGENTS.md` to your project root. JetBrains AI Assistant auto-detects `AGENTS.md` and `CLAUDE.md` at the project root and includes them in agent interactions:

```bash
curl -fsSL https://raw.githubusercontent.com/coree-ai/coree/main/integrations/jetbrains/AGENTS.md -o AGENTS.md
```

## Verify

Open an AI Assistant chat and ask:

```
call the diagnose tool and show me the output
```

## Notes

There is no distributable plugin format for MCP configs in JetBrains - config is imported manually or via the UI. No lifecycle hooks are exposed in the current AI Assistant API.
