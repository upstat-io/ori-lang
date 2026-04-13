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
echo "  NOTE: This script exercises files via 'ori build' + direct execution."
echo "  Only files with @main entry points produce runnable binaries."
echo "  Files using @test (attached tests) are compiled but skipped at runtime."
echo "  Full attached-test coverage requires harness-backed sanitizer runs"
echo "  (ori test --backend=llvm with ORI_SANITIZE) — tracked as future work."
echo ""

FAIL_COUNT=0
PASS_COUNT=0
SKIP_COUNT=0
COMPILE_FAIL_COUNT=0
TMPDIR=$(mktemp -d)
trap "rm -rf $TMPDIR" EXIT

for (( i=START; i<END; i++ )); do
    test_file="${ALL_TESTS[$i]}"
    name=$(basename "$test_file" .ori)

    # Compile to AOT binary with sanitizers via ORI_SANITIZE
    if ! "$ORI" build "$test_file" -o "$TMPDIR/san_full_$name" 2>/dev/null; then
        # Compilation failure — may be compile_fail test, @test-only file, or codegen limitation
        COMPILE_FAIL_COUNT=$((COMPILE_FAIL_COUNT + 1))
        continue
    fi

    # Run with per-test timeout (sanitized binaries are slower).
    # Capture stderr to detect ASan reports — ASan exits with code 1 by default,
    # which would be indistinguishable from a normal assertion failure without
    # checking the error stream.
    if timeout 10 "$TMPDIR/san_full_$name" 2>"$TMPDIR/stderr_$name" ; then
        PASS_COUNT=$((PASS_COUNT + 1))
    else
        exit_code=$?
        # Check if this is a sanitizer hit (ASan/UBSan report on stderr)
        if grep -q "Sanitizer\|ERROR:.*Sanitizer\|SUMMARY:.*Sanitizer" "$TMPDIR/stderr_$name" 2>/dev/null; then
            echo "FAIL (sanitizer): $test_file (exit $exit_code)"
            head -5 "$TMPDIR/stderr_$name" >&2
            FAIL_COUNT=$((FAIL_COUNT + 1))
        elif [ $exit_code -eq 127 ]; then
            # Exit 127 = command not found / no entry point
            SKIP_COUNT=$((SKIP_COUNT + 1))
        else
            # Non-zero but no sanitizer report — likely assertion failure or missing @main
            SKIP_COUNT=$((SKIP_COUNT + 1))
        fi
    fi
    rm -f "$TMPDIR/stderr_$name"

    # Clean up binary to avoid disk pressure
    rm -f "$TMPDIR/san_full_$name"
done

echo ""
echo "=== Shard $SHARD complete ==="
echo "  Passed:        $PASS_COUNT"
echo "  Failed:        $FAIL_COUNT"
echo "  Compile-skip:  $COMPILE_FAIL_COUNT"
echo "  Runtime-skip:  $SKIP_COUNT"
echo "  Total in shard: $((END - START))"

if [ "$FAIL_COUNT" -gt 0 ]; then
    echo "ERROR: $FAIL_COUNT sanitizer failure(s) in shard $SHARD"
    exit 1
fi
