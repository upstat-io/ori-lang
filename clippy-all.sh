#!/bin/bash
# Run clippy on Rust code.
#   ./clippy-all.sh                 # whole workspace (default)
#   ./clippy-all.sh -p ori_arc ...  # only the named crates (+ their deps)
# Scoped form is used by full-check.sh under ORI_COMMIT_SCOPE=staged so an
# unrelated check-in lints only the committing diff's crates.
set -e

if [ "$#" -gt 0 ]; then
    echo "=== Running clippy on selected crates: $* ==="
    cargo clippy "$@" --all-targets -- -D warnings
else
    echo "=== Running clippy on all crates ==="
    cargo clippy --workspace --all-targets -- -D warnings
fi

echo ""
echo "=== All clippy checks passed ==="
