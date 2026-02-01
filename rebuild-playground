#!/bin/bash
# Rebuild playground WASM and website
# Use --clean for a full clean rebuild (slower but guaranteed fresh)

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
WASM_DIR="$SCRIPT_DIR/website/playground-wasm"
PKG_DIR="$WASM_DIR/pkg"
WEBSITE_WASM_DIR="$SCRIPT_DIR/website/src/wasm"

# Check for --clean flag
if [[ "$1" == "--clean" ]]; then
    echo "=== Clean rebuild ==="
    cd "$WASM_DIR" && cargo clean
fi

echo "=== Building WASM ==="
cd "$WASM_DIR" && wasm-pack build --target web --out-dir pkg

echo "=== Copying to website ==="
mkdir -p "$WEBSITE_WASM_DIR"
cp "$PKG_DIR"/ori_playground_wasm.js "$WEBSITE_WASM_DIR/"
cp "$PKG_DIR"/ori_playground_wasm_bg.wasm "$WEBSITE_WASM_DIR/"
cp "$PKG_DIR"/ori_playground_wasm.d.ts "$WEBSITE_WASM_DIR/" 2>/dev/null || true
cp "$PKG_DIR"/ori_playground_wasm_bg.wasm.d.ts "$WEBSITE_WASM_DIR/" 2>/dev/null || true

echo "=== Building website ==="
cd "$SCRIPT_DIR/website" && bun run build

echo "=== Done ==="
