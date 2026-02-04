#!/bin/bash
# Run clippy on ALL Rust code: workspace and LLVM crate
# Usage: ./clippy-all

set -e

echo "=== Running clippy on workspace ==="
cargo cl

echo ""
echo "=== Running clippy on LLVM crate ==="
./llvm-clippy.sh

echo ""
echo "=== All clippy checks passed ==="
