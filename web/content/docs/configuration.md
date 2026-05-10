+++
title = "Configuration"
description = "coree is zero-config by default. All options are documented here."
weight = 10
template = "page.html"
+++

coree is zero-config by default. A `.coree.toml` file in your project root lets you override storage, remote sync, and indexing behaviour.

## File discovery

coree walks up from the current working directory looking for `.coree.toml`. The first file found wins. If none is found, the global config is used as a fallback:

- Linux: `~/.config/coree/config.toml`
- macOS: `~/Library/Application Support/coree/config.toml`
- Windows: `%APPDATA%\coree\config.toml`

The global config merges with the project config when both exist - useful for setting a Turso backend once across all projects.

## Environment variables

Any config field can be overridden via environment variable using `COREE__<SECTION>__<FIELD>` (double underscore as separator):

```
COREE__PROJECT_ID                -> project_id
COREE__PROJECT_ROOT              -> project_root
COREE__MEMORY__MODE              -> memory.mode
COREE__MEMORY__REMOTE_URL        -> memory.remote_url
COREE__MEMORY__REMOTE_AUTH_TOKEN -> memory.remote_auth_token
```

Env vars take precedence over both config files.

## Reference

### `project_id`

```toml
project_id = "my-project"
```

Scopes all memories to this identifier. Defaults to a hash of the project root path. Set this explicitly if you want memories to be portable across machines or directory locations.

---

### `project_root`

```toml
project_root = "/absolute/path/to/project"
```

Overrides the directory used for `.coree.toml` discovery and storage paths. Mainly useful for agent integrations that launch coree from a plugin directory.

---

### `[memory]`

#### `mode`

```toml
[memory]
mode = "managed"  # default
```

| Value | Description |
|-------|-------------|
| `managed` | Platform data directory, keyed by project path. No config required. |
| `local` | `local_path` relative to the project root. |
| `remote` | libSQL/Turso remote backend. Requires `remote_url`. |
| `disabled` | Memory subsystem disabled. No memory tools available. |

#### `local_path`

```toml
[memory]
mode = "local"
local_path = ".coree/memory.db"
```

#### `remote_url`

```toml
[memory]
mode = "remote"
remote_url = "libsql://your-db.turso.io"
```

#### `remote_mode`

```toml
[memory]
remote_mode = "replica"  # default: direct
```

| Value | Description |
|-------|-------------|
| `direct` | All reads and writes go directly to the remote. |
| `replica` | Maintains a local replica. Faster reads, works offline. |

#### `remote_auth_token`

Prefer the environment variable over the config file:

```bash
export COREE__MEMORY__REMOTE_AUTH_TOKEN=your-token
```

---

### `[index]`

#### `mode`

```toml
[index]
mode = "managed"  # default
```

Same values as `memory.mode`. Use `disabled` to turn off code indexing.

#### `git_history`

```toml
[index]
git_history = true  # default
```

Whether to index git commit history for churn analysis. Disabling speeds up initial index build on large repositories.

#### `exclude`

```toml
[index]
exclude = [
    "vendor/**",
    "third_party/**",
    "*.generated.go",
]
```

Additional glob patterns to exclude from indexing. Standard exclusions (`.git/`, `target/`, `node_modules/`) are always applied.

---

## Storage paths

In managed mode, databases are stored outside the project directory:

- Linux: `~/.local/share/coree/managed/<encoded-path>/`
- macOS: `~/Library/Application Support/coree/managed/<encoded-path>/`
- Windows: `%APPDATA%\coree\managed\<encoded-path>\`

`<encoded-path>` is the project root path with `/` replaced by `-`.

Each directory contains:
- `memory.db` - the memory database
- `index.db` - the code intelligence index

---

## Examples

### Local storage

```toml
[memory]
mode = "local"

[index]
mode = "local"
```

Everything stays in the project directory.

### Remote sync with Turso

`.coree.toml` (safe to commit):

```toml
project_id = "my-project"

[memory]
mode = "remote"
remote_mode = "replica"
remote_url = "libsql://your-db.turso.io"
```

`.envrc` (do not commit):

```bash
export COREE__MEMORY__REMOTE_AUTH_TOKEN=your-token
```

### Shared backend across all projects

`~/.config/coree/config.toml`:

```toml
[memory]
mode = "remote"
remote_mode = "replica"
remote_url = "libsql://your-db.turso.io"
```

Individual projects only need:

```toml
project_id = "my-project"
```
