#!/bin/bash
# Copy WASM build artifacts from playground/pkg to website/src/wasm
# Vite processes these as normal ESM imports (not static public assets).

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
WEBSITE_DIR="$(dirname "$SCRIPT_DIR")"
PKG_DIR="$WEBSITE_DIR/../playground/pkg"
WASM_DIR="$WEBSITE_DIR/src/wasm"

if [ ! -d "$PKG_DIR" ]; then
    echo "Error: playground/pkg not found at $PKG_DIR"
    echo "Build WASM first: cd playground/wasm && wasm-pack build --target web --out-dir ../pkg"
    exit 1
fi

mkdir -p "$WASM_DIR"

cp "$PKG_DIR/ori_playground_wasm.js" "$WASM_DIR/"
cp "$PKG_DIR/ori_playground_wasm_bg.wasm" "$WASM_DIR/"
cp "$PKG_DIR/ori_playground_wasm.d.ts" "$WASM_DIR/" 2>/dev/null || true
cp "$PKG_DIR/ori_playground_wasm_bg.wasm.d.ts" "$WASM_DIR/" 2>/dev/null || true

echo "WASM artifacts copied to $WASM_DIR"
ls -la "$WASM_DIR"
