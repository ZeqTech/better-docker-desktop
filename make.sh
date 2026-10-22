#!/usr/bin/env bash
# Usage:
#   ./make.sh            build for the current OS
#   ./make.sh win        Windows installer + portable .exe  (output in dist/)
#   ./make.sh mac        macOS universal .dmg (x64 + arm64) (must run on a Mac)
#   ./make.sh all        win, plus mac when running on a Mac
#   ./make.sh dev        run the app in development mode
#   ./make.sh release <patch|minor|major|X.Y.Z>  bump version, tag and push (starts the GitHub release)
# Env: MAC_TARGET=aarch64-apple-darwin builds one Mac architecture only; SKIP_TESTS=0 runs tests first.
cd "$(dirname "${BASH_SOURCE[0]:-$0}")" || exit 1

# Re-use the shell already running this script rather than resolving "bash" via PATH,
# which on Windows can pick the WSL launcher (C:\Windows\system32\bash.exe).
SHELL_BIN="${BASH:-sh}"

# Skip tests for local builds by default; run `SKIP_TESTS=0 ./make.sh` to include them.
export SKIP_TESTS="${SKIP_TESTS:-1}"
case "$1" in
    win|mac|all) exec "$SHELL_BIN" ./build.sh "$1" ;;
    release)     shift; exec "$SHELL_BIN" ./release.sh "$@" ;;
    dev)          exec "$SHELL_BIN" pnpm tauri dev ;;
    release)       exec "$SHELL_BIN" ./build.sh release ;;
    "")          exec "$SHELL_BIN" ./build.sh ;;
    *)           echo "Usage: $0 [win|mac|all|dev|release]" >&2; exit 2 ;;
esac
