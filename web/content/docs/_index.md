+++
title = "Introduction"
description = "coree documentation"
sort_by = "weight"
page_template = "page.html"
+++

coree is a local MCP server that gives AI agents persistent memory and code intelligence across sessions.

It solves a specific problem: agents forget everything when a session ends. Architecture decisions, discovered gotchas, how a subsystem works - all gone. The next session starts from scratch.

coree stores what agents learn and makes it searchable. At the start of each session, prior context is surfaced automatically. During work, a single `search()` call queries both stored memories and the indexed codebase simultaneously.

## Components

**Memory subsystem** - stores typed records: decisions, gotchas, how-it-works notes, facts. Each record has a title, content, importance score, and optional tags. Importance is used to rank results, not to filter them - store liberally.

**Code intelligence** - indexes source files using tree-sitter and git history. Tracks churn (how often a file or symbol changes), hotspot score (recent modification frequency), and cross-references between symbols.

**Hybrid search** - combines vector similarity search (semantic) with BM25 keyword search over both memories and code in a single ranked result set.

**Session context** - at session start, the agent calls `session_context()`. The most relevant memories for the current project are returned.

## Quick start

Install for your agent - see [Installation](/docs/installation/) - then add to your agent's system prompt or context file:

```
Before starting any task, call session_context() to load prior context.
Use search(query) before reading files or starting new work.
Store discoveries with store_memories() as you go.
```

coree works without configuration. Add a `.coree.toml` to your project root to change storage backend or enable remote sync. See [Configuration](/docs/configuration/).
