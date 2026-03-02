#!/bin/bash
# Run clippy on ALL Rust code
set -e

echo "=== Running clippy on all crates ==="
cargo clippy --workspace --all-targets -- -D warnings

echo ""
echo "=== All clippy checks passed ==="
