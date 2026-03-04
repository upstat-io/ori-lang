#!/bin/bash
# Format ALL Rust code: workspace and LLVM crate
# Usage: ./fmt-all.sh
#
# In pre-commit (lefthook stage_fixed: true), this checks first to report
# unformatted files, then auto-fixes so the commit proceeds with clean code.

set -e

echo "=== Formatting workspace ==="

# Check first — report what needs fixing
if ! cargo fmt --all -- --check 2>/dev/null; then
    echo ""
    echo "  Auto-fixing formatting..."
    cargo fmt --all
    echo "  Done — files were reformatted and will be re-staged."
fi

echo ""
echo "=== All formatting complete ==="
