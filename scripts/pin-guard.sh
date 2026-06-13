#!/usr/bin/env bash
set -euo pipefail

PLUGIN_VERSION=$(node -p "require('./package.json').version" 2>/dev/null || echo "")

if [ -z "$PLUGIN_VERSION" ]; then
  echo "WARNING: no package.json version found, skipping guard"
  exit 0
fi

PIN_VERSION=$(grep -rho '@coree-ai/coree@[0-9]\+\.[0-9]\+\.[0-9]\+' --include='*.json' --include='*.md' --include='*.rs' --include='*.ts' --include='*.js' --include='*.mjs' --include='*.toml' --include='*.yml' --include='*.yaml' . 2>/dev/null | head -1 | grep -o '[0-9]\+\.[0-9]\+\.[0-9]\+$' || echo "")

if [ -z "$PIN_VERSION" ]; then
  PIN_VERSION=$(grep -rho 'COREE_VERSION = "[0-9]\+\.[0-9]\+\.[0-9]\+"' --include='*.ts' --include='*.js' . 2>/dev/null | head -1 | grep -o '[0-9]\+\.[0-9]\+\.[0-9]\+' || echo "")
fi

if [ -z "$PIN_VERSION" ]; then
  echo "WARNING: no pin found, skipping guard"
  exit 0
fi

PIN_MAJOR_MINOR=$(echo "$PIN_VERSION" | cut -d. -f1-2)
PLUGIN_MAJOR_MINOR=$(echo "$PLUGIN_VERSION" | cut -d. -f1-2)

if [ "$PIN_MAJOR_MINOR" != "$PLUGIN_MAJOR_MINOR" ]; then
  echo "ERROR: pin/version mismatch"
  echo "  pin    = @coree-ai/coree@$PIN_VERSION (major.minor: $PIN_MAJOR_MINOR)"
  echo "  plugin = $PLUGIN_VERSION (major.minor: $PLUGIN_MAJOR_MINOR)"
  echo "  pin.major.minor ($PIN_MAJOR_MINOR) must equal plugin-version.major.minor ($PLUGIN_MAJOR_MINOR)"
  exit 1
fi

echo "OK: pin.major.minor ($PIN_MAJOR_MINOR) == plugin-version.major.minor ($PLUGIN_MAJOR_MINOR)"
