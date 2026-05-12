#!/usr/bin/env bash
# scripts/test-dist.sh
#
# Test the coree npm distribution from scratch, with no local binary overrides
# and no npm cache reuse. Validates the full download-and-run experience that
# a new user gets.
#
# Usage:
#   bash scripts/test-dist.sh               # test currently published version
#   bash scripts/test-dist.sh --local       # pack local build via pack-local.mjs, then test
#   bash scripts/test-dist.sh --latest      # test @latest from npm
#   bash scripts/test-dist.sh --keep        # keep temp dir on exit (for inspection)
#
# The --local mode still goes through the proper npm optionalDependencies
# resolution path (no COREE_BINARY_OVERRIDE). The binary inside the tarball
# is the local target/release/coree build. Use this to validate the packaging
# logic before publishing.
#
# Isolation strategy:
#   XDG_DATA_HOME   -> $WORK_DIR/data   (coree database and managed files)
#   XDG_CACHE_HOME  -> $WORK_DIR/cache  (ONNX/HF model cache on first run)
#   XDG_CONFIG_HOME -> $WORK_DIR/config (coree config lookup)
#   npm_config_cache -> $WORK_DIR/npm-cache  (npm download cache)
#   COREE_BINARY_OVERRIDE is unset so the npm optionalDependencies path is used.
#   Real HOME is never touched.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MAIN_PKG="$REPO_ROOT/npm/@coree-ai/coree/package.json"
PUBLISHED_VERSION="$(node -p "require('$MAIN_PKG').version")"

MODE="published"
KEEP=0
PKG_REF="@coree-ai/coree@$PUBLISHED_VERSION"

for arg in "$@"; do
  case "$arg" in
    --local)  MODE="local" ;;
    --latest) PKG_REF="@coree-ai/coree@latest" ;;
    --keep)   KEEP=1 ;;
    *) echo "Unknown argument: $arg"; exit 1 ;;
  esac
done

WORK_DIR="$(mktemp -d /tmp/coree-dist-test.XXXXXX)"
cleanup() { if [[ $KEEP -eq 0 ]]; then rm -rf "$WORK_DIR"; fi; }
trap cleanup EXIT

NPM_CACHE="$WORK_DIR/npm-cache"
mkdir -p "$NPM_CACHE" "$WORK_DIR/data" "$WORK_DIR/cache" "$WORK_DIR/config"

# Unset any override that would bypass the npm optionalDependencies resolution.
unset COREE_BINARY_OVERRIDE 2>/dev/null || true

echo "==> coree distribution test"
echo "    Work dir : $WORK_DIR"
echo "    Mode     : $MODE"

if [[ "$MODE" == "local" ]]; then
  echo ""
  echo "==> Building local tarballs (pack-local.mjs)..."
  node "$REPO_ROOT/scripts/pack-local.mjs"
  TGZ="$(ls -t "$REPO_ROOT/tmp/npm"/coree-ai-coree-*-local.tgz | head -1)"
  PKG_REF="file:$TGZ"
fi

echo "    Package  : $PKG_REF"
echo ""

# Run npx with a completely isolated environment. XDG vars redirect all storage
# so we test the first-run experience without touching the real home directory.
run_npx() {
  XDG_DATA_HOME="$WORK_DIR/data" \
  XDG_CACHE_HOME="$WORK_DIR/cache" \
  XDG_CONFIG_HOME="$WORK_DIR/config" \
  npm_config_cache="$NPM_CACHE" \
    npx --yes "$PKG_REF" "$@"
}

# -- Test 1: --version -------------------------------------------------------
# Verifies the binary resolves through optionalDependencies and executes.
echo "---- Test 1: coree --version ----"
run_npx --version
echo ""

# -- Test 2: inject (hooks path) ---------------------------------------------
# Simulates the SessionStart hook. With no server running, inject exits 0 and
# emits the not-running message (which Claude uses to tell the user to wait).
echo "---- Test 2: coree inject --type session ----"
run_npx inject --type session 2>&1 || true
echo ""

# -- Test 3: serve MCP handshake ---------------------------------------------
# Start `coree serve`, send a JSON-RPC initialize request, expect a result.
# The initialize response comes before the ONNX model loads, so this completes
# within seconds even on a first run. Proves the binary resolves and the MCP
# transport works end-to-end.
echo "---- Test 3: coree serve (MCP initialize handshake) ----"
INIT='{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"dist-test","version":"0.0.1"}}}'

SERVE_OUT="$(
  run_npx serve 2>/dev/null \
    < <(printf '%s\n' "$INIT"; sleep 10) \
    || true
)"

if printf '%s' "$SERVE_OUT" | grep -q '"result"'; then
  echo "PASS: got JSON-RPC result"
  printf '%s\n' "$SERVE_OUT" | head -5
else
  echo "FAIL: no JSON-RPC result in output"
  echo "--- raw output (first 30 lines) ---"
  printf '%s\n' "$SERVE_OUT" | head -30
  echo "---"
  exit 1
fi
echo ""

echo "==> All tests passed."
if [[ $KEEP -eq 1 ]]; then echo "    Work dir kept: $WORK_DIR"; fi
