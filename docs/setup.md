# Setup Guide

## 1. Build and install the binary

```bash
cargo install --path .
```

This puts `memso` in `~/.cargo/bin/`. Make sure that is in your `$PATH`.

## 2. Configure Claude Code

```bash
memso install
```

This adds the MCP server and hooks to `~/.claude/settings.json` automatically.
It is safe to run multiple times - already-configured entries are skipped.

Use `memso install --dry-run` to preview changes before writing.

Restart Claude Code after running install.

## 3. Add memory instructions to your project's CLAUDE.md

Copy the contents of `docs/rules/CLAUDE.md` into your project's CLAUDE.md,
replacing `<project>` with your project name.

## 4. Optional: enable Turso Cloud sync

To sync memories across machines, create a `.memso.toml` in your project root:

```toml
[backend]
mode = "replica"
remote_url = "libsql://your-db.turso.io"
auth_token = "${MEMSO_AUTH_TOKEN}"
```

Set `MEMSO_AUTH_TOKEN` in your environment (e.g. via `.envrc` with direnv).

Add `.memso.db` to `.gitignore`. The `.memso.toml` can be committed if you
use the `${ENV_VAR}` form for the token.

## Per-project scoping

memso uses the current working directory basename as the project ID by default.
To set an explicit ID, add to `.memso.toml`:

```toml
[memory]
project_id = "my-project"
```

Or set `$MEMSO_PROJECT` in your environment.
