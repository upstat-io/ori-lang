#!/bin/bash
# Run clippy on Rust code.
#   ./clippy-all.sh                 # whole workspace (default)
#   ./clippy-all.sh -p ori_arc ...  # only the named crates (+ their deps)
# Scoped form is used by full-check.sh under ORI_COMMIT_SCOPE=staged so an
# unrelated check-in lints only the committing diff's crates.
set -e

MESSAGE_FORMAT_ARGS=()
if [[ -n "${ORI_CLIPPY_MESSAGE_FORMAT:-}" ]]; then
    MESSAGE_FORMAT_ARGS+=("--message-format=${ORI_CLIPPY_MESSAGE_FORMAT}")
fi
LINT_ARGS=(-- -D warnings)
if [[ "${ORI_CLIPPY_CAPTURE_DIAGNOSTICS:-0}" == "1" ]]; then
    LINT_ARGS=()
fi

if [ "$#" -gt 0 ]; then
    echo "=== Running clippy on selected crates: $* ==="
    cargo clippy "${MESSAGE_FORMAT_ARGS[@]}" "$@" --all-targets "${LINT_ARGS[@]}"
else
    echo "=== Running clippy on all crates ==="
    cargo clippy "${MESSAGE_FORMAT_ARGS[@]}" --workspace --all-targets "${LINT_ARGS[@]}"
fi

echo ""
echo "=== All clippy checks passed ==="
