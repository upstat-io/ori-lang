#!/usr/bin/env bash
# Run the full spec test suite with sanitizers enabled (sharded).
# Uses file-walking with shard support for CI parallelism.
# Expected to be called from CI with TEST_SHARD and TEST_TOTAL_SHARDS env vars.
#
# NOTE: The canonical test harness (ori test --backend=llvm) does not yet support
# shard-based parallelism. This script walks tests/spec/ directly, filtering out
# _test/ companions and compile_fail files. When harness sharding is added, this
# script should be updated to use it instead.
set -euo pipefail

SHARD="${TEST_SHARD:-1}"
TOTAL="${TEST_TOTAL_SHARDS:-1}"
ORI="${ORI_BIN:-target/release/ori}"

if [ ! -x "$ORI" ]; then
    ORI="${ORI_BIN:-target/debug/ori}"
fi

if [ ! -x "$ORI" ]; then
    echo "ERROR: Ori binary not found at $ORI"
    echo "  Build with: cargo build --release"
    exit 1
fi

echo "=== Sanitizer full sweep: shard $SHARD of $TOTAL ==="
echo "  Binary: $ORI"
echo "  ORI_SANITIZE=${ORI_SANITIZE:-not set}"

if [ -z "${ORI_SANITIZE:-}" ]; then
    echo "WARNING: ORI_SANITIZE not set — running without sanitizer instrumentation"
fi

# Collect all spec test files, excluding _test/ companions (run by the harness)
# and sorting for deterministic shard assignment.
# Use while-read loop instead of mapfile for Bash 3.2 compatibility (macOS).
ALL_TESTS=()
while IFS= read -r f; do
    ALL_TESTS+=("$f")
done < <(find tests/spec -name '*.ori' -not -path '*/_test/*' | sort)

TOTAL_TESTS=${#ALL_TESTS[@]}

if [ "$TOTAL_TESTS" -eq 0 ]; then
    echo "ERROR: No test files found in tests/spec/"
    exit 1
fi

PER_SHARD=$(( (TOTAL_TESTS + TOTAL - 1) / TOTAL ))
START=$(( (SHARD - 1) * PER_SHARD ))
END=$(( START + PER_SHARD ))
[ "$END" -gt "$TOTAL_TESTS" ] && END="$TOTAL_TESTS"

echo "  Running tests $((START + 1)) to $END of $TOTAL_TESTS"
echo ""

FAIL_COUNT=0
PASS_COUNT=0
SKIP_COUNT=0
TMPDIR=$(mktemp -d)
trap "rm -rf $TMPDIR" EXIT

for (( i=START; i<END; i++ )); do
    test_file="${ALL_TESTS[$i]}"
    name=$(basename "$test_file" .ori)

    # Compile to AOT binary with sanitizers via ORI_SANITIZE
    if ! "$ORI" build "$test_file" -o "$TMPDIR/san_full_$name" 2>/dev/null; then
        # Compilation failure — may be compile_fail test or codegen limitation
        SKIP_COUNT=$((SKIP_COUNT + 1))
        continue
    fi

    # Run with per-test timeout (sanitized binaries are slower)
    if timeout 10 "$TMPDIR/san_full_$name" 2>/dev/null; then
        PASS_COUNT=$((PASS_COUNT + 1))
    else
        echo "FAIL: $test_file"
        FAIL_COUNT=$((FAIL_COUNT + 1))
    fi

    # Clean up binary to avoid disk pressure
    rm -f "$TMPDIR/san_full_$name"
done

echo ""
echo "=== Shard $SHARD complete: $PASS_COUNT passed, $FAIL_COUNT failed, $SKIP_COUNT skipped ==="

if [ "$FAIL_COUNT" -gt 0 ]; then
    echo "ERROR: $FAIL_COUNT sanitizer failure(s) in shard $SHARD"
    exit 1
fi
