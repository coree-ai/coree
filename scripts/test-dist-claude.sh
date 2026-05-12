#!/usr/bin/env bash
# scripts/test-dist-claude.sh
#
# Launch Claude Code with a completely fresh environment and the coree plugin
# loaded, simulating the first-time-user install experience:
#
#   - Fresh HOME: no existing ~/.claude config, sessions, or installed plugins
#   - Fresh npm cache: npx downloads the binary from npm (no cache reuse)
#   - Fresh XDG dirs: coree starts with no prior data or model cache
#   - COREE_BINARY_OVERRIDE unset: binary resolves via npm optionalDependencies
#
# You will be prompted to log in - that is intentional (fresh config).
# The coree MCP server starts via npx on first use, which may take a moment.
#
# Usage:
#   bash scripts/test-dist-claude.sh               # published plugin
#   bash scripts/test-dist-claude.sh --local       # pack and use local build
#   bash scripts/test-dist-claude.sh --keep        # keep temp dir after exit

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

MODE="published"
KEEP=0
PLUGIN_DIR="$REPO_ROOT/agents/claude"

for arg in "$@"; do
  case "$arg" in
    --local) MODE="local" ;;
    --keep)  KEEP=1 ;;
    *) echo "Unknown argument: $arg"; exit 1 ;;
  esac
done

if [[ "$MODE" == "local" ]]; then
  echo "==> Building local tarballs (pack-local.mjs)..."
  node "$REPO_ROOT/scripts/pack-local.mjs"
  PLUGIN_DIR="$REPO_ROOT/agents/claude-local/plugin"
fi

WORK_DIR="$(mktemp -d /tmp/coree-claude-test.XXXXXX)"
cleanup() { if [[ $KEEP -eq 0 ]]; then rm -rf "$WORK_DIR"; fi; }
trap cleanup EXIT

mkdir -p \
  "$WORK_DIR/claude-home" \
  "$WORK_DIR/data" \
  "$WORK_DIR/cache" \
  "$WORK_DIR/config" \
  "$WORK_DIR/npm-cache"

unset COREE_BINARY_OVERRIDE 2>/dev/null || true

echo "==> Launching Claude with a clean environment"
echo "    Mode       : $MODE"
echo "    Plugin dir : $PLUGIN_DIR"
echo "    Work dir   : $WORK_DIR"
echo ""
echo "    You will be asked to log in (expected - fresh HOME)."
echo "    The coree MCP server starts automatically via the plugin MCP config."
echo ""

HOME="$WORK_DIR/claude-home" \
XDG_DATA_HOME="$WORK_DIR/data" \
XDG_CACHE_HOME="$WORK_DIR/cache" \
XDG_CONFIG_HOME="$WORK_DIR/config" \
npm_config_cache="$WORK_DIR/npm-cache" \
  claude --plugin-dir "$PLUGIN_DIR"

if [[ $KEEP -eq 1 ]]; then
  echo ""
  echo "Work dir kept: $WORK_DIR"
  echo "  Claude config : $WORK_DIR/claude-home/.claude/"
  echo "  Coree data    : $WORK_DIR/data/"
  echo "  npm cache     : $WORK_DIR/npm-cache/"
fi
