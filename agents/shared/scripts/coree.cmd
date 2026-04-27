#!/usr/bin/env sh
:; DIR=$(cd "$(dirname "$0")/.." && pwd); case "$(uname -s)" in Darwin) B="$DIR/bin/coree-bootstrap-macos";; Linux) case "$(uname -m)" in x86_64) B="$DIR/bin/coree-bootstrap-linux-x86_64";; aarch64|arm64) B="$DIR/bin/coree-bootstrap-linux-aarch64";; *) echo "coree: unsupported arch $(uname -m)" >&2; exit 1;; esac;; MINGW*|MSYS*|CYGWIN*) B="$DIR/bin/coree-bootstrap-windows.exe";; *) echo "coree: unsupported OS $(uname -s)" >&2; exit 1;; esac; chmod +x "$B" 2>/dev/null; exec "$B" "$@"
@echo off
"%~dp0..\bin\coree-bootstrap-windows.exe" %*
