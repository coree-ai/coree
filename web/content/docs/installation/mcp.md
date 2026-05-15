+++
title = "Any MCP client"
description = "Add coree to any MCP-compatible agent."
weight = 60
template = "page.html"
+++

coree runs as a stdio MCP server. Any MCP-compatible agent or client can connect to it.

## MCP server config

```json
{
  "mcpServers": {
    "coree": {
      "command": "npx",
      "args": ["--yes", "@coree-ai/coree@0.14.1", "serve"]
    }
  }
}
```

Add this to whatever config format your MCP client uses. The server communicates over stdio using the standard MCP protocol.

## Direct invocation

Run the server directly if your client accepts a command and arguments:

```
command: npx
args:    --yes @coree-ai/coree@0.14.1 serve
```

Or, if you have the binary installed:

```
command: coree
args:    serve
```

## Environment variables

Pass environment variables to the server process to configure storage or remote sync:

```json
{
  "mcpServers": {
    "coree": {
      "command": "npx",
      "args": ["--yes", "@coree-ai/coree@0.14.1", "serve"],
      "env": {
        "COREE__MEMORY__MODE": "remote",
        "COREE__MEMORY__REMOTE_URL": "libsql://your-db.turso.io",
        "COREE__MEMORY__REMOTE_AUTH_TOKEN": "your-token"
      }
    }
  }
}
```

## Project root

coree determines the project root by walking up from the working directory looking for `.coree.toml` or `.git`. If your MCP client does not set the working directory to your project root, set it explicitly:

```json
{
  "env": {
    "COREE__PROJECT_ROOT": "/path/to/your/project"
  }
}
```

## Protocol version

coree implements MCP 2024-11-05. It uses stdio transport only.
