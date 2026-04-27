# Configuration

coree is zero-config by default. A `.coree.toml` file in your project root lets you override storage, remote sync, and indexing behaviour.

## File discovery

coree walks up from the current directory looking for `.coree.toml`. The first file found wins. If none is found, a global config at the platform config directory is used as a fallback:

- Linux: `~/.config/coree/config.toml`
- macOS: `~/Library/Application Support/coree/config.toml`
- Windows: `%APPDATA%\coree\config.toml`

The global config is merged with the project config when both exist - useful for setting a Turso backend once across all projects.

## Environment variables

Any config field can be overridden via environment variable using `COREE__<SECTION>__<FIELD>` (double underscore as separator):

```
COREE__PROJECT_ID                -> project_id
COREE__MEMORY__MODE              -> memory.mode
COREE__MEMORY__REMOTE_URL        -> memory.remote_url
COREE__MEMORY__REMOTE_AUTH_TOKEN -> memory.remote_auth_token
```

Env vars take precedence over both the global and project config files.

## Reference

### `project_id`

```toml
project_id = "my-project"
```

Scopes all memories to this identifier. Defaults to a hash of the project root path. Set this explicitly if you want memories to be portable across machines or directory locations.

---

### `[memory]`

Controls where the memory database is stored.

#### `mode`

```toml
[memory]
mode = "managed"  # default
```

| Value | Description |
|-------|-------------|
| `managed` | Stored in the platform data directory, keyed by project path. No configuration required. |
| `local` | Stored at `local_path` relative to the project root. |
| `remote` | libSQL remote backend (Turso). Requires `remote_url`. |
| `disabled` | Memory subsystem entirely disabled. No tools available. |

#### `managed_path`

Override the base directory for managed-mode storage:

```toml
[memory]
managed_path = "/data/coree"
```

#### `local_path`

Path for `local` mode. Relative paths are resolved from the project root:

```toml
[memory]
mode = "local"
local_path = ".coree/memory.db"  # default when mode = local
```

#### `remote_url`

Required when `mode = "remote"`. The libSQL/Turso database URL:

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
| `direct` | All reads and writes go directly to the remote. No local copy. |
| `replica` | Maintains a local replica that syncs to remote. Faster reads, works offline. |

#### `remote_auth_token`

Auth token for the Turso database. Prefer the environment variable over putting this in the config file:

```
COREE__MEMORY__REMOTE_AUTH_TOKEN=your-token
```

---

### `[index]`

Controls the code intelligence index.

#### `mode`

```toml
[index]
mode = "managed"  # default
```

Accepts the same values as `memory.mode`. Use `disabled` to turn off code indexing entirely.

#### `git_history`

```toml
[index]
git_history = true  # default
```

Whether to index git commit history for churn analysis. Disabling speeds up the initial index build on large repositories.

#### `exclude`

Additional glob patterns to exclude from indexing:

```toml
[index]
exclude = [
    "vendor/**",
    "third_party/**",
    "*.generated.go",
]
```

---

## Storage paths

By default (managed mode), databases are stored outside the project directory:

- Linux: `~/.local/share/coree/managed/<encoded-path>/`
- macOS: `~/Library/Application Support/coree/managed/<encoded-path>/`
- Windows: `%APPDATA%\coree\managed\<encoded-path>\`

Where `<encoded-path>` is the project root path with `/` replaced by `-` (e.g. `/home/user/myproject` becomes `-home-user-myproject`).

Each project directory contains:
- `memory.db` - the memory database
- `index.db` - the code intelligence index

---

## Examples

### Local storage (keep everything in the project)

```toml
[memory]
mode = "local"

[index]
mode = "local"
```

### Remote sync with Turso (replica mode)

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

### Shared backend across all projects (global config)

`~/.config/coree/config.toml`:
```toml
[memory]
mode = "remote"
remote_mode = "replica"
remote_url = "libsql://your-db.turso.io"
```

Individual projects only need a `.coree.toml` with a `project_id`:
```toml
project_id = "my-project"
```
