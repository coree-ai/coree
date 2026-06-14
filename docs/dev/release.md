# Release Process

## Prerequisites

- CI green on `main`
- Push access to `coree-ai/coree`
- Clean working tree on `main` (release-it commits, tags, and pushes)

## Version numbering

| Component | Example | Notes |
|-----------|---------|-------|
| Binary + npm packages | `0.15.0` | Matches `Cargo.toml` exactly |
| Model package | `1.0.0` | Independent of binary. Only change when the bundled model changes. |

The per-editor **plugin configs** (`@coree-ai/coree@<ver>` pins in the `claude`, `gemini`, `opencode`, etc. repos) are **not** bumped here. The `Release` workflow calls `propagate.yml`, which bumps those pins, tags each plugin repo, and (for `opencode`) publishes to npm automatically. See [step 3](#3-let-the-release-workflow-run).

## Steps

### 1. Bump and tag with release-it

The bump is automated by [`release-it`](https://github.com/release-it/release-it) (config in `.release-it.json`). From a clean `main`:

```bash
npm run release -- 0.15.0 --ci      # or `npm run release` for interactive prompts
```

This single command:

- Bumps the version in `Cargo.toml` and all five `npm/@coree-ai/*/package.json` files (including the four `optionalDependencies` refs in the main package)
- Updates root `package.json` + `package-lock.json`
- Runs the `after:bump` hook (`scripts/bump-web-docs.mjs`) to update `web/config.toml` and every `web/content/docs/installation/*.md` npx pin
- Generates `CHANGELOG.md` from conventional commits
- Commits `chore: release 0.15.0`, tags `v0.15.0`, and **pushes the commit and tag**

It does **not** publish to npm or create the GitHub release (`npm.publish` and `github.release` are both `false`) — the `Release` workflow does that on the tag push.

> A docs-only push to `main` is ignored by `dev-release.yml`, but the release commit touches `Cargo.toml`/`npm/`, so the dev-release will also fire on the commit push. That is harmless; the tagged `Release` workflow is the real release.

### 2. Determine the new version

If unsure what to bump to:

```bash
git tag --sort=-version:refname | head -5
```

Increment from the latest tag per semver (minor for features, patch for fixes).

### 3. Let the `Release` workflow run

Pushing the `v*` tag triggers the `Release` workflow:

1. **Build jobs** (parallel): Linux x86_64, Linux aarch64, macOS aarch64, Windows x86_64 (~5-7 min)
2. **publish-npm job** (after builds): runs `generate-npm-packages.mjs`, publishes the four platform packages, then the main `@coree-ai/coree` package last (with npm provenance)
3. **build-web / deploy-web jobs**: rebuild and deploy the docs site
4. **propagate job** (after npm publish): calls `propagate.yml`, a matrix over the plugin repos (`antigravity`, `claude`, `codex`, `gemini`, `openclaw`, `opencode`, `zed`). For each it rewrites the `@coree-ai/coree` pin, syncs the plugin's own version on a major/minor bump, refreshes the lockfile, commits to `main`, and pushes a `v<plugin-version>` tag (which fires `opencode`'s `publish.yml`)

Total: ~12-15 minutes plus propagation time.

### 4. Verify the release

```bash
gh release view v0.15.0 --repo coree-ai/coree
npm view @coree-ai/coree version
npm view @coree-ai/coree-linux-x64 version
```

### 5. Verify plugin propagation

The `propagate` job bumps the plugin repos directly (no PRs to merge). Spot-check:

```bash
gh api repos/coree-ai/opencode/git/refs/tags/v0.15.0 --jq .ref   # tag exists
npm view @coree-ai/opencode version                              # opencode published
```

If a plugin repo's `main` has branch protection, `coree-release-bot` must be on the bypass list or the propagation push is rejected.

---

## Plugin pin propagation

Plugin repos pin a specific coree binary version (`@coree-ai/coree@<ver>`). These pins are bumped by **`propagate.yml`** (called by the `Release` workflow after npm publish) — there is no manual plugin-config step. The shared script `scripts/propagate-coree-pin.mjs` rewrites the pin wherever it appears (generic, no per-repo config) and syncs each plugin's own version to `<coree-major>.<coree-minor>.0` on a major/minor coree bump; the workflow commits and pushes a `v<plugin-version>` tag using the `coree-release-bot` app token (the app-token push is what triggers `opencode`'s `publish.yml` — a `GITHUB_TOKEN` push would not). Only `opencode` is an npm package; the other 6 repos are git/marketplace-distributed, so their commit + tag is the release artifact. The `coree-version-check` reusable workflow guards that pin major.minor == plugin major.minor.

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
