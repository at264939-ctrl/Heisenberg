#!/usr/bin/env bash
set -euo pipefail

# ═══════════════════════════════════════════════════════════════════════
# cook.sh — Operational helper commands for Heisenberg
# Execution wrappers, environment tooling, and memory-capped runners.
# ═══════════════════════════════════════════════════════════════════════

ROOT_DIR=$(cd "$(dirname "$0")/.." && pwd)

usage() {
    echo "Usage: cook.sh <command> [args...]"
    echo ""
    echo "Commands:"
    echo "  run [--mem-mb <MB>] -- <command>   Run with memory cap (default: 3072 MB)"
    echo "  clean                              Clean old task scripts"
    echo "  sysinfo                            Show system info"
    echo "  status                             Query agent memory via IPC"
}

if [ "$#" -eq 0 ]; then
    usage; exit 1
fi

cmd="$1"; shift

case "$cmd" in
    run)
        MEM_MB=3072
        if [ "${1:-}" = "--mem-mb" ]; then
            MEM_MB="$2"; shift 2
        fi
        if [ "${1:-}" = "--" ]; then shift; fi

        if [ "$#" -eq 0 ]; then
            echo "Error: No command specified after 'run'" >&2
            exit 1
        fi

        ULIMIT_KB=$((MEM_MB * 1024))
        echo "⚗  Running: $* (cap: ${MEM_MB}MB)"
        ulimit -v "$ULIMIT_KB"
        exec "$@"
        ;;
    clean)
        echo "Cleaning old task scripts..."
        find "$ROOT_DIR" -name "heisenberg_task*.sh" -mtime +1 -delete 2>/dev/null || true
        echo "✓ Done"
        ;;
    sysinfo)
        echo "╭─ System Info ─────────────────────────╮"
        echo "│ OS:       $(uname -s) $(uname -m)"
        if command -v free &>/dev/null; then
            echo "│ RAM Free: $(free -m | awk '/^Mem:/{print $4}') MB"
            echo "│ RAM Used: $(free -m | awk '/^Mem:/{print $3}') MB"
            echo "│ RAM Total:$(free -m | awk '/^Mem:/{print $2}') MB"
        fi
        echo "│ Cores:    $(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo '?')"
        echo "╰───────────────────────────────────────╯"
        ;;
    status)
        FIFO="/tmp/heisenberg_cmd.fifo"
        echo '{"cmd":"mike_status"}' > "$FIFO"
        cat /tmp/heisenberg_resp.fifo
        ;;
    *)
        usage; exit 1
        ;;
esac
