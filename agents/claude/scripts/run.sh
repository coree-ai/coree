#!/usr/bin/env bash
# Wrapper used by both .mcp.json and hooks to launch the memso binary.
# Honours MEMSO_BINARY_OVERRIDE for dev/custom builds; falls back to the
# binary installed by bootstrap.sh in CLAUDE_PLUGIN_DATA.
#
# On a cold install the binary may not yet exist when this script runs —
# bootstrap.sh downloads it in parallel. Poll for up to 30 s before giving up.
BINARY="${MEMSO_BINARY_OVERRIDE:-${CLAUDE_PLUGIN_DATA}/memso}"

# On a cold install bootstrap.sh downloads the binary in parallel with this
# script starting. Poll until it appears (or .exe variant on Windows) for up
# to 300 s — sized for a 20 MB binary on a 1 Mbit/s connection with margin.
if [[ ! -f "${BINARY}" && ! -f "${BINARY}.exe" ]]; then
  for i in $(seq 1 600); do
    sleep 0.5
    { [[ -f "${BINARY}" ]] || [[ -f "${BINARY}.exe" ]]; } && break
  done
fi

exec "${BINARY}" "$@"
