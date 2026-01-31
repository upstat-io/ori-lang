#!/bin/bash
# Copy WASM build artifacts from playground/pkg to website
# - JS/TS files go to src/wasm/ for Vite to bundle
# - WASM binary goes to public/wasm/ as a static asset

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
WEBSITE_DIR="$(dirname "$SCRIPT_DIR")"
PKG_DIR="$WEBSITE_DIR/../playground/pkg"
WASM_DIR="$WEBSITE_DIR/src/wasm"
PUBLIC_WASM_DIR="$WEBSITE_DIR/public/wasm"

if [ ! -d "$PKG_DIR" ]; then
    echo "Error: playground/pkg not found at $PKG_DIR"
    echo "Build WASM first: cd playground/wasm && wasm-pack build --target web --out-dir ../pkg"
    exit 1
fi

mkdir -p "$WASM_DIR"
mkdir -p "$PUBLIC_WASM_DIR"

# Type definitions go to src/wasm for IDE support
cp "$PKG_DIR/ori_playground_wasm.d.ts" "$WASM_DIR/" 2>/dev/null || true
cp "$PKG_DIR/ori_playground_wasm_bg.wasm.d.ts" "$WASM_DIR/" 2>/dev/null || true

# JS and WASM binary go to public for static serving
# The dynamic import loads these at runtime from /wasm/
cp "$PKG_DIR/ori_playground_wasm.js" "$PUBLIC_WASM_DIR/"
cp "$PKG_DIR/ori_playground_wasm_bg.wasm" "$PUBLIC_WASM_DIR/"

echo "Type definitions copied to $WASM_DIR"
echo "JS + WASM binary copied to $PUBLIC_WASM_DIR"
ls -la "$WASM_DIR"
ls -la "$PUBLIC_WASM_DIR"
