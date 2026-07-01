+++
title = "opencode"
description = "Add coree to opencode with the official plugin."
weight = 35
template = "page.html"
+++

coree ships an official opencode plugin. Installing it sets up the MCP server **and** the lifecycle hooks in one step.

## Install

```bash
opencode plugin @coree-ai/opencode --global
```

This downloads the plugin and adds it to your global opencode config:

- Linux: `~/.config/opencode/opencode.json`
- macOS: `~/.config/opencode/opencode.json`
- Windows: `%USERPROFILE%\.config\opencode\opencode.json`

```json
{
  "plugin": [
    "@coree-ai/opencode"
  ]
}
```

That single entry registers the coree MCP server and wires up context injection - no separate MCP block to maintain.

To install for a single project instead, omit `--global` and the entry is added to the project's `opencode.json`.

## Hooks

The plugin injects relevant memories on session start and live memory/code suggestions on every prompt via opencode's `chat.message` hook, and re-injects context after the conversation is compacted. No configuration required.

## Agent instructions (optional)

For explicit usage guidance, add `opencode.md` to your project root (or `~/.config/opencode/AGENTS.md` for all projects):

```bash
# Linux / macOS
curl -fsSL https://raw.githubusercontent.com/coree-ai/opencode/main/opencode.md \
  -o opencode.md
```

```powershell
# Windows (PowerShell)
Invoke-WebRequest -Uri "https://raw.githubusercontent.com/coree-ai/opencode/main/opencode.md" -OutFile opencode.md
```

## Updating

```bash
opencode plugin @coree-ai/opencode --global --force
```

The `--force` flag replaces the existing version. Without it, opencode sees the plugin already in your config and does nothing.

opencode does not auto-update npm plugins. The installed version is cached and is not re-resolved from the npm registry on subsequent starts. The cache location is:

- Linux: `~/.cache/opencode/packages/@coree-ai/opencode@latest/`
- macOS: `~/.cache/opencode/packages/@coree-ai/opencode@latest/`
- Windows: `%USERPROFILE%\.cache\opencode\packages\@coree-ai\opencode@latest\`

If a new coree version has been published and the plugin still reports the old version after running the update command, see [Troubleshooting](#troubleshooting) below.

## Verify

Start an opencode session and run:

```
call the diagnose tool
```

The `diagnose` MCP tool reports server state, database status, and any initialisation errors.

## Troubleshooting

**Plugin does not update after running `opencode plugin --force`:**
opencode caches npm plugins and may not re-resolve `latest` from the registry. Remove the cached package and restart opencode:

```bash
# Linux / macOS
rm -rf ~/.cache/opencode/packages/@coree-ai

# Windows (PowerShell)
Remove-Item -Recurse -Force "$env:USERPROFILE\.cache\opencode\packages\@coree-ai"
```

On next startup, opencode re-installs `@coree-ai/opencode@latest` from the npm registry.

**MCP server does not appear in the tool list:**
Restart opencode completely. If the server still does not appear, check that `"@coree-ai/opencode"` is present in the `"plugin"` array of your config file (see paths above).

**Server starts but times out on first use:**
First-run downloads the platform binary and embedding model, which can take 30-90 seconds. Wait for the download to complete - subsequent starts are fast.

**npx hangs or fails:**
Ensure Node.js 18+ is installed. Check that your network allows npm registry access (`registry.npmjs.org`).
