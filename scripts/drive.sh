#!/usr/bin/env bash
set -euo pipefail

# ═══════════════════════════════════════════════════════════════════════
# drive.sh — Browser sandbox controller for Heisenberg
# Provides a sandboxed, allow-listed browser control interface.
# ═══════════════════════════════════════════════════════════════════════

ALLOWED_HOSTS=("localhost" "127.0.0.1" "example.com")

usage() {
    echo "Usage: drive.sh <command> <args>"
    echo ""
    echo "Commands:"
    echo "  open <url>       Open URL (must be in allowed hosts)"
    echo "  click <selector> Simulate click (headless placeholder)"
}

if [ "$#" -lt 2 ]; then
    usage; exit 1
fi

cmd="$1"; shift

case "$cmd" in
    open)
        url="$1"
        host=$(echo "$url" | sed -E 's|https?://([^/:]+).*|\1|')
        ok=false
        for a in "${ALLOWED_HOSTS[@]}"; do
            if [[ "$host" == *"$a"* ]]; then ok=true; break; fi
        done
        if [ "$ok" = true ]; then
            echo "🌐 Opening: $url"
            xdg-open "$url" >/dev/null 2>&1 || open "$url" 2>/dev/null || echo "Headless open: $url"
        else
            echo "✖ Host blocked by sandbox: $host" >&2
            exit 2
        fi
        ;;
    click)
        selector="$1"
        echo "👆 Simulating click: $selector"
        # Placeholder for headless browser integration
        ;;
    *)
        usage; exit 1
        ;;
esac
