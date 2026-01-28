#!/bin/bash
# Format ALL Rust code: workspace and LLVM crate
# Usage: ./fmt-all

set -e

echo "=== Formatting workspace ==="
cargo fmt --all

echo ""
echo "=== Formatting LLVM crate ==="
cargo fmt --manifest-path compiler/ori_llvm/Cargo.toml

echo ""
echo "=== All formatting complete ==="
