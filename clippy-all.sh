#!/bin/bash
# Run clippy on ALL Rust code: workspace, runtime, and LLVM crate
# Usage: ./clippy-all

set -e

echo "=== Running clippy on workspace ==="
cargo cl

echo ""
echo "=== Running clippy on runtime library ==="
cargo clippy --manifest-path compiler/ori_rt/Cargo.toml --all-targets -- -D warnings

echo ""
echo "=== Running clippy on LLVM crate ==="
./llvm-clippy.sh

echo ""
echo "=== All clippy checks passed ==="
