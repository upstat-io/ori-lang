#!/bin/bash
# Build ALL Rust code: workspace including LLVM crate
# Usage: ./build-all
set -e

echo "=== Building workspace ==="
cargo build --workspace

echo ""
echo "=== All builds complete ==="
