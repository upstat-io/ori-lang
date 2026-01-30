#!/bin/bash
# Rebuild WASM and copy to website for dev iteration.
# Usage: ./scripts/rebuild-wasm.sh
#
# After running this, refresh the playground page to pick up changes.

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
WEBSITE_DIR="$(dirname "$SCRIPT_DIR")"
WASM_DIR="$WEBSITE_DIR/../playground/wasm"

echo "Building WASM..."
cd "$WASM_DIR"
wasm-pack build --target web --release --out-dir ../pkg

echo ""
echo "Copying to website..."
"$SCRIPT_DIR/copy-wasm.sh"

echo ""
echo "Done! Refresh the playground page to pick up changes."
