# Release Process

## Prerequisites

- All milestone issues closed
- CI green on `main`
- Push access to `coree-ai/coree`

## Version numbering

| Component | Example | Notes |
|-----------|---------|-------|
| Binary + npm packages | `0.9.1` | Matches `Cargo.toml` exactly |
| Plugin configs | `0.9.1-1` | `<binary-version>-<plugin-patch>`. Increment the plugin patch (e.g. `0.9.1-1` -> `0.9.1-2`) when changing hook/MCP config **without** a binary release. Reset to `-1` on each new binary version. |
| Model package | `1.0.0` | Independent of binary. Only change when the bundled model changes. |

## Steps

### 1. Determine the new version

```bash
git tag --sort=-version:refname | head -5
gh api repos/coree-ai/coree/milestones | jq '.[].title'
```

The open milestone title is the target version. If no milestone exists, increment the minor version from the latest tag.

### 2. Bump versions

Update **every** location listed below. Missing any one will cause a publish failure or version mismatch.

**`Cargo.toml`** — binary version (source of truth):
```toml
version = "0.9.1"
```

**npm packages** — main package and all four platform packages:
```
npm/@coree-ai/coree/package.json          (version + all optionalDependencies refs)
npm/@coree-ai/coree-linux-x64/package.json
npm/@coree-ai/coree-linux-arm64/package.json
npm/@coree-ai/coree-darwin-arm64/package.json
npm/@coree-ai/coree-win32-x64/package.json
```

**Plugin configs** — agent integration metadata (plugin-patch resets to 1):
```
agents/claude/.claude-plugin/plugin.json   (e.g. "0.9.1-1")
agents/gemini/gemini-extension.json        (e.g. "0.9.1-1")
```

Update the lockfile:
```bash
cargo check
```

### 3. Commit, push, and tag simultaneously

```bash
git add Cargo.toml Cargo.lock \
  npm/@coree-ai/coree/package.json \
  npm/@coree-ai/coree-linux-x64/package.json \
  npm/@coree-ai/coree-linux-arm64/package.json \
  npm/@coree-ai/coree-darwin-arm64/package.json \
  npm/@coree-ai/coree-win32-x64/package.json \
  agents/claude/.claude-plugin/plugin.json \
  agents/gemini/gemini-extension.json
git commit -m "chore: bump version to 0.9.1"
git push
git tag v0.9.1
git push origin v0.9.1
```

The commit and tag can be pushed together. The `Release` workflow triggers on the tag and builds fresh from source regardless.

### 4. Wait for the Release workflow

Pushing the tag triggers the `Release` workflow:

1. **Build jobs** (parallel): Linux x86_64, Linux aarch64, macOS aarch64, Windows x86_64 (~5-7 min)
2. **publish-npm job** (sequential, after builds):
   - Checks if model package version is new on npm — skips fetch/publish if already published
   - Publishes platform packages (`@coree-ai/coree-linux-x64` etc.)
   - Publishes main package (`@coree-ai/coree`) last

Total: ~12-15 minutes.

### 5. Verify the release

```bash
gh release view v0.9.1 --repo coree-ai/coree
npm view @coree-ai/coree version
npm view @coree-ai/coree-linux-x64 version
```

### 6. Close the milestone

Close the GitHub milestone once the release workflow is green.

---

## Plugin-only releases

If only agent config files change (hooks, MCP settings) with no binary change:

1. Increment the plugin patch in `agents/claude/.claude-plugin/plugin.json` and `agents/gemini/gemini-extension.json` (e.g. `0.9.1-1` -> `0.9.1-2`).
2. Commit and push to `main`. No tag needed — no binary or npm publish occurs.

---

## Model package releases

The `@coree-ai/coree-model-bge-small-en-v1.5` package is versioned independently of
the binary. It is only republished when the bundled model changes. See
[npm-distribution.md](npm-distribution.md#updating-the-model-package) for the procedure.

The `model.yml` workflow handles dev packs of the model package automatically whenever
`scripts/fetch-model.py` or `npm/@coree-ai/coree-model-bge-small-en-v1.5/` changes.

## Dev releases

Every push to `main` (that touches `src/`, `Cargo.toml`, `Cargo.lock`, `npm/`, or `scripts/`) produces updated dev artifacts on the `dev` GitHub release:
- Platform binaries (`coree-linux-x86_64.tar.gz` etc.)
- npm tarballs for binary + main packages (`coree-ai-coree-X.X.X-dev.N.tgz` etc.)
- Model npm tarball (`coree-ai-coree-model-bge-small-en-v1.5-1.0.0.tgz`) — only when model files change

Install the dev build in Claude Code:
```
/plugin install coree-dev
```
