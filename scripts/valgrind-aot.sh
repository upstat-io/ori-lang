#!/bin/bash
# Run AOT-compiled Ori programs under Valgrind to detect memory errors.
#
# Usage:
#   ./scripts/valgrind-aot.sh                  # Run all .ori files in tests/valgrind/
#   ./scripts/valgrind-aot.sh path/to/file.ori # Run a specific file
#
# Requires: valgrind, ori (with LLVM support)
#
# Checks for:
#   - Use-after-free (invalid reads/writes)
#   - Double-free
#   - Memory leaks (via Valgrind, not ORI_CHECK_LEAKS)
#   - Buffer overflows
#
# Exit codes:
#   0 = all tests clean
#   1 = memory errors detected

set -euo pipefail

PASS=0
FAIL=0
SKIP=0
ERRORS=()

run_one() {
    local ori_file="$1"
    local name
    name=$(basename "$ori_file" .ori)
    local tmpdir
    tmpdir=$(mktemp -d)
    local binary="$tmpdir/$name"

    # Compile
    if ! ori build "$ori_file" -o "$binary" 2>"$tmpdir/compile.log"; then
        echo "  SKIP  $name (compile failed)"
        SKIP=$((SKIP + 1))
        rm -rf "$tmpdir"
        return
    fi

    # Run under Valgrind (|| true prevents set -e from aborting on non-zero)
    local val_log="$tmpdir/valgrind.log"
    local exit_code
    exit_code=$(ORI_CHECK_LEAKS=1 valgrind \
        --leak-check=full \
        --show-leak-kinds=definite,indirect \
        --errors-for-leak-kinds=definite \
        --error-exitcode=42 \
        --log-file="$val_log" \
        "$binary" >/dev/null 2>&1; echo $?) || true

    if [ "$exit_code" -eq 0 ]; then
        echo "  PASS  $name"
        PASS=$((PASS + 1))
    elif [ "$exit_code" -eq 42 ]; then
        echo "  FAIL  $name (memory errors)"
        grep -E "(definitely lost|Invalid|ERROR SUMMARY)" "$val_log" | sed 's/^/        /'
        FAIL=$((FAIL + 1))
        ERRORS+=("$name")
    else
        echo "  FAIL  $name (exit code $exit_code)"
        FAIL=$((FAIL + 1))
        ERRORS+=("$name")
    fi

    rm -rf "$tmpdir"
}

echo "=== Valgrind AOT Memory Check ==="
echo ""

if [ $# -gt 0 ]; then
    for f in "$@"; do
        run_one "$f"
    done
else
    test_dir="tests/valgrind"
    if [ ! -d "$test_dir" ]; then
        echo "No tests/valgrind/ directory. Run with a specific .ori file."
        exit 0
    fi
    for f in "$test_dir"/*.ori; do
        [ -f "$f" ] || continue
        run_one "$f"
    done
fi

echo ""
echo "=== Results: $PASS passed, $FAIL failed, $SKIP skipped ==="

if [ ${#ERRORS[@]} -gt 0 ]; then
    echo "Failures: ${ERRORS[*]}"
    exit 1
fi
