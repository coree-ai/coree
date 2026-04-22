## Persistent Memory and Code Intelligence

You have persistent memory and indexed code search across sessions via the tyto MCP server.

### Searching — always start here

Before starting any task, and before reading any file not yet examined this session:

  search(query="<topic>")

This searches memory, source code, and git history simultaneously. It is the default
entry point for all lookups. Use it before reaching for Read or grep.

For exact symbol lookups (faster and more precise than search):

  get_symbol(name="function_or_struct_name")

For code-only results when memory noise would obscure what you need:

  search_code(query="<topic>")

For memory-only results (decisions, gotchas, preferences):

  search_memory(query="<topic>")

Call get_memories(ids=[<id>]) to read the full content of any result that looks relevant.

### Storing memories

Call store_memories when you:
- Learn something non-obvious about this project's architecture or conventions
- Make or discover an architectural decision or trade-off
- Find the solution to a non-obvious or recurring problem
- Encounter a gotcha (something that breaks in a non-obvious way)
- Observe a preference or constraint the user has expressed

Do NOT store: obvious facts, temporary state, things derivable from reading the
code, or anything already documented in CLAUDE.md.

### Field guidance

- type: choose the most specific type from:
  decision, gotcha, problem-solution, how-it-works, what-changed,
  trade-off, preference, discovery, workflow, fact
- topic_key: a short stable slug for the subject, e.g. "auth-session-store",
  "time-crate-policy". Memories with the same topic_key are updated in place
  rather than duplicated.
- importance: 0.0-1.0
  - 0.9+ for architectural decisions affecting the whole system
  - 0.7+ for gotchas, non-obvious constraints, security-relevant facts
  - 0.5  for useful context, patterns, preferences
  - 0.3  for supplementary facts
- facts: array of short discrete statements, e.g.
  ["Uses tower-sessions-sqlx-store 0.15.0", "PostgresStore auto-migrates on startup"]

### Notes on code search

search() degrades gracefully to memory-only if the index is not yet ready.
The index builds in the background on startup; code results populate as files are processed.
Only Rust and Python source files are indexed. Markdown and config files are not.
