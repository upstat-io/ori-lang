#!/bin/bash
# Format ALL Rust code: workspace and LLVM crate
# Usage: ./fmt-all

set -e

echo "=== Formatting workspace ==="
cargo fmt --all

echo ""
echo "=== All formatting complete ==="
