# Rename Checklist

Everything that references the name "coree" and will need updating when the project is renamed.
Replace `<new>` with the chosen name and `<new-ai>` with the new GitHub org/npm scope equivalent.

---

## Repository / Version Control

| What | Current value | Notes |
|------|--------------|-------|
| GitHub repository | `coree-ai/coree` | Rename repo and org on GitHub |
| Local working directory | `~/Development/coree` | Move/rename after repo rename |
| Git remote URLs in package.json files | `https://github.com/coree-ai/coree.git` | 5x package.json files in `npm/` |

---

## Rust Crate (`Cargo.toml`)

| Field | Current value |
|-------|--------------|
| `name` | `"coree"` |
| `description` | references "coree" and "Turso" |
| Binary name (`[[bin]]` implied) | `coree` |
| Bootstrap binary name | `coree-bootstrap` |
| Env var prefix in comment | `COREE__SECTION__FIELD` |

Downstream: `use coree::...` in all `src/*.rs` and `tests/db.rs` will change to match new crate name.

---

## Source Code (`src/`)

| File | References |
|------|-----------|
| `src/main.rs` | `#[command(name = "coree", ...)]`, `use coree::...`, user-facing strings `"coree inject error"`, `"coree request error"`, MCP server key `"coree"`, help strings |
| `src/install.rs` | `MCP_SERVER_NAME = "coree"`, JSON keys `mcpServers.coree`, help/error strings |
| `src/serve.rs` | `struct CoreeServer`, `Implementation::new("coree", ...)`, log strings `"coree: ..."`, `"coree-serve.log"`, tool description strings, `MigrateToTursoInput` struct name (Turso is a dependency name, not ours -- keep as-is unless desired) |
| `src/status.rs` | `println!("coree v{}", ...)`, `"run 'coree install'"` |
| `src/embed.rs` | `COREE_MODEL_DIR`, `COREE_FORCE_MODEL_REFRESH`, `[coree]` log prefix, `.join("coree")` cache subdir |
| `src/log.rs` | `[coree]` log prefix |
| `src/migrations.rs` | `[coree] WARNING` log prefix |
| `src/bin/coree-bootstrap.rs` | `const REPO: &str = "coree-ai/coree"`, download URL template, log prefixes `"coree stub: ..."`, binary name `exe("coree")` |
| `src/project_id.rs` | Unit test fixture URLs `coree-ai/coree` |

---

## Environment Variables (user-visible, must be documented)

All existing users will need to update these in their shell configs / CI:

| Current | Rename to |
|---------|-----------|
| `COREE__*` (config prefix, e.g. `COREE__MEMORY__MODE`) | `<NEW>__*` |
| `COREE_BINARY_OVERRIDE` | `<NEW>_BINARY_OVERRIDE` |
| `COREE_CHANNEL` | `<NEW>_CHANNEL` |
| `COREE_MODEL_DIR` | `<NEW>_MODEL_DIR` |
| `COREE_FORCE_MODEL_REFRESH` | `<NEW>_FORCE_MODEL_REFRESH` |
| `COREE_PLUGIN_DATA` | `<NEW>_PLUGIN_DATA` |

---

## Config Files (user-visible, stored on disk)

| Current path/filename | Notes |
|----------------------|-------|
| `.coree.toml` | Project config file name; users have this committed in their repos |
| `~/.config/coree/config.toml` (Linux) | Global config dir |
| `~/Library/Application Support/coree/config.toml` (macOS) | Global config dir |
| `%APPDATA%\coree\config.toml` (Windows) | Global config dir |
| Managed DB path `...coree/managed/<encoded-path>/` | Data dir inside XDG dirs |
| Default local DB `".coree/memory.db"` | Relative path default |
| Log file `coree-serve.log` | Written to XDG data dir |

Migration note: existing users' `.coree.toml` files and data directories will need to be moved or the binary will need a compatibility shim.

---

## Agent Plugin Files

### `agents/claude/`
| File | References |
|------|-----------|
| `.claude-plugin/plugin.json` | `"name": "coree"`, GitHub URL |
| `.mcp.json` | MCP server key `"coree"` |
| `hooks/hooks.json` | Script path `coree.cmd`, hook names imply coree |

### `agents/gemini/`
| File | References |
|------|-----------|
| `gemini-extension.json` | `"name": "coree"`, MCP key `"coree"`, `COREE_PLUGIN_DATA` env, description |
| `GEMINI.md` | Tool names `mcp_coree_search`, `mcp_coree_store_memories`, `mcp_coree_capture_note` (these are derived from the MCP server name -- will auto-update if server name changes) |
| `hooks/hooks.json` | Script path `coree.cmd`, hook names |

### `agents/shared/scripts/`
| File | References |
|------|-----------|
| `coree.cmd` | Script filename; referenced in hooks; error messages `"coree: unsupported arch"` |

### `agents/shared/bin/`
| Files | Notes |
|-------|-------|
| `coree-bootstrap-linux-x86_64` | Binary filename (committed stub) |
| `coree-bootstrap-linux-aarch64` | Binary filename (committed stub) |
| `coree-bootstrap-macos` | Binary filename (committed stub) |
| `coree-bootstrap-windows.exe` | Binary filename (committed stub) |

---

## npm Packages

### Directory structure to rename
```
npm/@coree/            -> npm/@<new-ai>/
  coree/                  -> <new>/
  coree-linux-x64/        -> <new>-linux-x64/
  coree-linux-arm64/      -> <new>-linux-arm64/
  coree-darwin-arm64/     -> <new>-darwin-arm64/
  coree-win32-x64/        -> <new>-win32-x64/
```

### Files to update
| File | References |
|------|-----------|
| `npm/@coree-ai/coree/package.json` | `"name"`, `"bin"` key, `optionalDependencies` keys, GitHub URL |
| `npm/@coree-ai/coree/.claude-plugin/plugin.json` | `"name"`, GitHub URLs |
| `npm/@coree-ai/coree/.mcp.json` | MCP server key `"coree"` |
| `npm/@coree-ai/coree/hooks/hooks.json` | `bin/coree` paths in commands |
| `npm/@coree-ai/coree-linux-x64/package.json` | `"name"`, `"description"`, GitHub URL |
| `npm/@coree-ai/coree-linux-arm64/package.json` | same |
| `npm/@coree-ai/coree-darwin-arm64/package.json` | same |
| `npm/@coree-ai/coree-win32-x64/package.json` | same |

### Binary filename inside npm package
`npm/@coree-ai/coree/bin/coree` (the Node.js launcher script) and the platform binaries it resolves.

---

## GitHub Actions Workflows

| File | References |
|------|-----------|
| `.github/workflows/release.yml` | Artifact names `coree-*.tar.gz / .zip`, binary name, npm package paths |
| `.github/workflows/dev-release.yml` | Same artifact/binary names, release notes mention plugin install slug |
| `.github/workflows/e2e.yml` | Artifact/binary names, env vars `COREE_BIN`, `COREE_BINARY_OVERRIDE`, config file `.coree.toml` |
| `.github/workflows/ci.yml` | Binary names in matrix, artifact upload name `coree-debug-*` |
| `.github/workflows/build-stubs.yml` | Bootstrap binary names, artifact names, commit message references |

---

## Claude Marketplace Plugin

| File | Field | Current value |
|------|-------|--------------|
| `.claude-plugin/marketplace.json` | `"name"` | `"coree"` |
| `.claude-plugin/marketplace.json` | `plugins[].name` | `"coree"` |
| `.claude-plugin/marketplace.json` | `plugins[].source.package` | `"@coree-ai/coree"` |
| `.claude-plugin/marketplace.json` | `plugins[].homepage` | `"https://github.com/coree-ai/coree"` |

This file controls the Claude Code `/plugin install` slug. Changing it will break existing install instructions pointing to the old name.

---

## Project Config File (repo root)

| File | References |
|------|-----------|
| `.coree.toml` | `project_id = "coree"`, Turso `remote_url` (contains personal DB name, not project name) |

---

## Gitleaks Config

| File | Field | Current value |
|------|-------|--------------|
| `gitleaks.toml` | `description` | `"Coree project allowlist"` |
| `gitleaks.toml` | rule `id` | `"coree-credential-env"` |
| `gitleaks.toml` | `regex` | `COREE_(TOKEN|AUTH|...)` |
| `gitleaks.toml` | `tags` | `"coree"` |

---

## Dev Environment

| File | References |
|------|-----------|
| `devenv.nix` | `COREE_BINARY_OVERRIDE`, `COREE_CHANNEL`, `RUST_LOG = "coree=debug"` |

---

## Documentation

| File | Notes |
|------|-------|
| `docs/user/config.md` | All env var names, config paths, `.coree.toml` references |
| `docs/dev/release.md` | GitHub org/repo `coree-ai/coree` |
| `docs/dev/npm-distribution.md` | npm scope `@coree-ai`, all package names, env vars, binary names |
| `CONTRIBUTING.md` | Project name in heading |

---

## Scripts

| File | References |
|------|-----------|
| `scripts/generate-npm-packages.mjs` | npm scope `@coree-ai`, package names, binary filename `coree${ext}` |

---

## External Services (actions required outside the repo)

| Service | Current name/handle | Action required |
|---------|-------------------|-----------------|
| **GitHub org** | `coree-ai` | Rename org or create new org and transfer repo |
| **GitHub repo** | `coree-ai/coree` | Rename (GitHub preserves redirects, but release download URLs in bootstrap binary are hardcoded) |
| **GitHub Releases** | Artifact names `coree-*.tar.gz` | New releases will use new names; old stubs in `agents/shared/bin/` point to old URLs until rebuilt |
| **npm registry** | `@coree-ai` scope, packages `@coree-ai/coree` and 4x platform packages | Create new scope, publish under new names; cannot rename existing npm packages |
| **Claude Code plugin marketplace** | Listed as `coree`, install slug from `marketplace.json` | Submit updated `marketplace.json` to Anthropic; old install instructions will break |
| **crates.io** | Not yet published (no `[package].publish` key, no workflow step) | Reserve the new crate name before publishing |
| **Turso database** | DB named `memory-memso-beefsack` (personal instance, not project-named) | No rename needed |

---

## Bootstrap Binary Download URL (critical)

The committed bootstrap binaries in `agents/shared/bin/` and the npm platform stubs contain hardcoded GitHub release URLs constructed from `REPO = "coree-ai/coree"` in `src/bin/coree-bootstrap.rs`. After the rename:

1. The new binary must be built and released under the new repo/org name.
2. New bootstrap stubs must be committed that reference the new URL.
3. Old stubs already distributed to users will still try to download from `coree-ai/coree`. GitHub redirects the repo URL but release asset URLs are **not** redirected -- old stubs will break unless a redirect or compatibility release is published under the old name.
