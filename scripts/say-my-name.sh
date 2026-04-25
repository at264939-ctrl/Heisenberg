#!/usr/bin/env bash
set -euo pipefail

# ═══════════════════════════════════════════════════════════════════════
# say-my-name.sh — Terminal entrypoint for Heisenberg
# "Say my name." — Launches the Heisenberg autonomous agent.
# ═══════════════════════════════════════════════════════════════════════

ROOT_DIR=$(cd "$(dirname "$0")/.." && pwd)
INSTALL_BIN="$HOME/.local/bin/Heisenberg"
LOCAL_BIN="$ROOT_DIR/target/release/heisenberg"

if [ -x "$INSTALL_BIN" ]; then
  exec "$INSTALL_BIN" "$@"
elif [ -x "$LOCAL_BIN" ]; then
  exec "$LOCAL_BIN" "$@"
else
  echo "Heisenberg binary not found." >&2
  echo "Run: bash scripts/build.sh" >&2
  exit 1
fi
