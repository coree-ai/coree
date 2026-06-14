+++
title = "opencode"
description = "Add coree to opencode with the official plugin."
weight = 35
template = "page.html"
+++

coree ships an official opencode plugin. Installing it sets up the MCP server **and** the lifecycle hooks in one step.

## Install

```bash
opencode plugin @coree-ai/opencode
```

This downloads the plugin and adds it to your opencode config (`~/.config/opencode/opencode.json` on Linux by default):

```json
{
  "plugin": [
    "@coree-ai/opencode"
  ]
}
```

That single entry registers the coree MCP server and wires up context injection - no separate MCP block to maintain.

## Hooks

The plugin injects relevant memories on session start and live memory/code suggestions on every prompt via opencode's `chat.message` hook, and re-injects context after the conversation is compacted. No configuration required.

## Agent instructions (optional)

For explicit usage guidance, add `opencode.md` to your project root (or `~/.config/opencode/AGENTS.md` for all projects):

```bash
curl -fsSL https://raw.githubusercontent.com/coree-ai/opencode/main/opencode.md \
  -o opencode.md
```

## Verify

Start an opencode session and run:

```
call the diagnose tool
```

The `diagnose` MCP tool reports server state, database status, and any initialisation errors.
