#!/bin/bash
# Copy WASM build artifacts from playground-wasm/pkg to src/wasm/
# All files go to src/wasm/ so Vite can properly bundle them

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
WEBSITE_DIR="$(dirname "$SCRIPT_DIR")"
PKG_DIR="$WEBSITE_DIR/playground-wasm/pkg"
WASM_DIR="$WEBSITE_DIR/src/wasm"

if [ ! -d "$PKG_DIR" ]; then
    echo "Error: playground-wasm/pkg not found at $PKG_DIR"
    echo "Build WASM first: cd website/playground-wasm && wasm-pack build --target web --out-dir pkg"
    exit 1
fi

mkdir -p "$WASM_DIR"

# Copy all WASM artifacts to src/wasm/ for Vite to process
cp "$PKG_DIR/ori_playground_wasm.d.ts" "$WASM_DIR/" 2>/dev/null || true
cp "$PKG_DIR/ori_playground_wasm_bg.wasm.d.ts" "$WASM_DIR/" 2>/dev/null || true
cp "$PKG_DIR/ori_playground_wasm.js" "$WASM_DIR/"
cp "$PKG_DIR/ori_playground_wasm_bg.wasm" "$WASM_DIR/"

echo "WASM artifacts copied to $WASM_DIR"
ls -la "$WASM_DIR"
