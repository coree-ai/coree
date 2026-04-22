#!/usr/bin/env sh
:; DIR=$(cd "$(dirname "$0")/.." && pwd); case "$(uname -s)" in Darwin) B="$DIR/bin/tyto-bootstrap-macos";; Linux) case "$(uname -m)" in x86_64) B="$DIR/bin/tyto-bootstrap-linux-x86_64";; aarch64|arm64) B="$DIR/bin/tyto-bootstrap-linux-aarch64";; *) echo "tyto: unsupported arch $(uname -m)" >&2; exit 1;; esac;; MINGW*|MSYS*|CYGWIN*) B="$DIR/bin/tyto-bootstrap-windows.exe";; *) echo "tyto: unsupported OS $(uname -s)" >&2; exit 1;; esac; chmod +x "$B" 2>/dev/null; exec "$B" "$@"
@echo off
"%~dp0..\bin\tyto-bootstrap-windows.exe" %*
