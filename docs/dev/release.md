# Release Process

## Prerequisites

- All milestone issues closed
- CI green on `main`
- Push access to `tyto-ai/tyto`

## Steps

### 1. Determine the new version

Check the most recent tag and any open milestones to determine the correct version:

```bash
git tag --sort=-version:refname | head -5
gh api repos/tyto-ai/tyto/milestones | jq '.[].title'
```

The open milestone title is the target version. If no milestone exists, increment the minor version from the latest tag.

### 2. Bump versions

`Cargo.toml` and all agent plugin manifests need updating. Plugin versions use a `-N` suffix — reset to `-1` on a new tyto release, or increment if patching the plugin only.

**`Cargo.toml`:**
```
version = "0.8.0"
```

**Plugin manifests (update all):**
```
agents/claude/.claude-plugin/plugin.json
agents/gemini/gemini-extension.json
```

Each takes the form `"version": "0.8.0-1"`.

Update the lockfile:

```bash
cargo check
```

### 3. Commit and push

```bash
git add Cargo.toml Cargo.lock agents/claude/.claude-plugin/plugin.json agents/gemini/gemini-extension.json
git commit -m "chore: bump version to 0.8.0"
git push
```

### 4. Wait for CI

The `Build Bootstrap Binaries` workflow runs on every push to `main` and commits updated bootstrap binaries back to the repo. **Wait for all CI workflows to go green before tagging.**

### 5. Pull, tag, and push

```bash
git pull --rebase   # picks up the bootstrap binary commit from CI
git tag v0.8.0
git push origin v0.8.0
```

The `pull --rebase` is required. CI commits updated bootstrap binaries after the push in step 2; the tag must point to that commit, not the version bump commit.

### 6. Wait for the Release workflow

Pushing the tag triggers the `Release` workflow, which builds platform binaries and attaches them to the GitHub release. This takes roughly 12 minutes (4 parallel builds: Linux x86_64, Linux aarch64, macOS aarch64, Windows x86_64).

### 7. Close the milestone

Close the GitHub milestone once the release workflow is green.
