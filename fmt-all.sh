#!/bin/bash
# Format ALL Rust code: workspace and LLVM crate
# Usage: ./fmt-all.sh
#
# In pre-commit (lefthook), this lists the files rustfmt would change across the
# WHOLE workspace, formats them, and stages exactly those files — including ones
# not already staged. The lefthook `stage_fixed` only re-stages already-staged
# files, so an unstaged-but-unformatted file would otherwise reach HEAD unformatted
# and fail CI's `cargo fmt --all -- --check`. Staging only rustfmt's own change set
# (via `--check -l`) avoids sweeping unrelated working-tree edits.

set -e

echo "=== Formatting workspace ==="

# `--check -l` prints the files that WOULD be reformatted, without changing them.
FMT_LIST="$(cargo fmt --all -- --check -l 2>/dev/null || true)"

if [ -n "$FMT_LIST" ]; then
    COUNT=$(printf '%s\n' "$FMT_LIST" | grep -c .)
    echo ""
    echo "  Reformatting $COUNT file(s)..."
    cargo fmt --all
    # Stage exactly the files rustfmt changed.
    printf '%s\n' "$FMT_LIST" | while IFS= read -r f; do
        [ -n "$f" ] && git add -- "$f" 2>/dev/null || true
    done
    echo "  Done — reformatted and staged $COUNT file(s)."
fi

echo ""
echo "=== All formatting complete ==="
