#!/usr/bin/env bash
set -euo pipefail

# ═══════════════════════════════════════════════════════════════════════
# build.sh — System build & validation for Heisenberg
# Verifies toolchain, models, llama-server, and builds the Rust core.
# ═══════════════════════════════════════════════════════════════════════

ROOT_DIR=$(cd "$(dirname "$0")/.." && pwd)
MODELS_DIR="$ROOT_DIR/models"
INSTALL_BIN_DIR="$HOME/.local/bin"

echo ""
echo "  ╔════════════════════════════════════════╗"
echo "  ║   HEISENBERG — Chemistry Setup         ║"
echo "  ╚════════════════════════════════════════╝"
echo ""

# ── 1. Rust Toolchain ─────────────────────────────────────────────────
echo "  [1/5] Checking Rust toolchain..."
command -v rustc >/dev/null 2>&1 || { echo "  ✖ rustc not found. Install Rust: https://rustup.rs" >&2; exit 1; }
command -v cargo >/dev/null 2>&1 || { echo "  ✖ cargo not found. Install Rust: https://rustup.rs" >&2; exit 1; }
echo "  ✓ Rust $(rustc --version | awk '{print $2}')"

# ── 2. Model Files ───────────────────────────────────────────────────
echo "  [2/5] Scanning for GGUF models..."
if [ ! -d "$MODELS_DIR" ]; then
  echo "  ✖ models/ directory missing. Create $MODELS_DIR and add .gguf files." >&2
  exit 1
fi

GGUF_COUNT=$(find "$MODELS_DIR" -maxdepth 1 -name "*.gguf" 2>/dev/null | wc -l)
if [ "$GGUF_COUNT" -eq 0 ]; then
  echo "  ✖ No .gguf model files found in models/" >&2
  echo "  Place a GGUF model in the models/ directory." >&2
  exit 1
else
  echo "  ✓ Found $GGUF_COUNT GGUF model(s):"
  find "$MODELS_DIR" -maxdepth 1 -name "*.gguf" -exec basename {} \; | sed 's/^/    /'
fi

# ── 3. llama-server ──────────────────────────────────────────────────
echo "  [3/5] Checking llama-server..."
if command -v llama-server &>/dev/null; then
  echo "  ✓ llama-server found in PATH"
elif [ -f "$ROOT_DIR/bin/llama-server" ]; then
  echo "  ✓ llama-server found at bin/llama-server"
else
  echo "  ⚠ llama-server not found. Building from source..."
  mkdir -p "$ROOT_DIR/bin"
  git clone --depth=1 https://github.com/ggerganov/llama.cpp.git "$ROOT_DIR/.llama_cpp_build"
  cd "$ROOT_DIR/.llama_cpp_build"
  cmake -DBUILD_SHARED_LIBS=OFF -B build
  cmake --build build --config Release -j"$(nproc)" --target llama-server
  cp build/bin/llama-server "$ROOT_DIR/bin/"
  cd "$ROOT_DIR"
  rm -rf "$ROOT_DIR/.llama_cpp_build"
  echo "  ✓ llama-server built and installed to bin/"
fi

# ── 4. Build Rust Core ───────────────────────────────────────────────
echo "  [4/5] Compiling Heisenberg (release)..."
cd "$ROOT_DIR"
cargo build --release

# ── 5. Install CLI ───────────────────────────────────────────────────
echo "  [5/5] Installing CLI..."
BIN_PATH="$ROOT_DIR/target/release/heisenberg"
if [ ! -x "$BIN_PATH" ]; then
  echo "  ✖ Build succeeded but binary not found at $BIN_PATH" >&2
  exit 1
fi

mkdir -p "$INSTALL_BIN_DIR"
cp "$BIN_PATH" "$INSTALL_BIN_DIR/Heisenberg"
chmod +x "$INSTALL_BIN_DIR/Heisenberg"

echo ""
echo "  ╔════════════════════════════════════════╗"
echo "  ║   ✓ Build Complete                     ║"
echo "  ╠════════════════════════════════════════╣"
echo "  ║   Models:  $GGUF_COUNT GGUF model(s) ready"
echo "  ║   Engine:  llama-server ready"
echo "  ║   Quant:   TurboQuant integrated"
echo "  ╠════════════════════════════════════════╣"
echo "  ║   Run: Heisenberg chat                 ║"
echo "  ╚════════════════════════════════════════╝"
echo ""

# Ensure PATH includes install dir
if [[ ":$PATH:" != *":$INSTALL_BIN_DIR:"* ]]; then
  echo "  Add to PATH: export PATH=\"$INSTALL_BIN_DIR:\$PATH\""
fi
