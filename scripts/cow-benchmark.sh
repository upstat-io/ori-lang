#!/usr/bin/env bash
# COW Benchmark Runner
#
# Compiles and runs all COW benchmark programs, reporting times.
# Measures both compile time and runtime for each benchmark.
#
# Usage: ./scripts/cow-benchmark.sh [--release]
#
# Requires: the LLVM-enabled ori binary

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
BENCH_DIR="${ROOT_DIR}/tests/benchmarks/cow"
TMPDIR="${ROOT_DIR}/build/cow-benchmarks"
mkdir -p "$TMPDIR"

# Use release binary if --release is passed
ORI="${ROOT_DIR}/target/debug/ori"
MODE="debug"
if [[ "${1:-}" == "--release" ]]; then
    ORI="${ROOT_DIR}/target/release/ori"
    MODE="release"
fi

if [[ ! -x "$ORI" ]]; then
    echo "ERROR: $ORI not found. Run 'cargo bl' (or 'cargo blr' for release) first."
    exit 1
fi

if [[ ! -d "$BENCH_DIR" ]]; then
    echo "ERROR: $BENCH_DIR not found."
    exit 1
fi

echo "============================================"
echo "  COW Benchmark Suite ($MODE)"
echo "  Date: $(date -Iseconds)"
echo "  Binary: $ORI"
echo "============================================"
echo

# --- Compile + Run ---
printf "%-25s %10s %10s %8s %6s\n" "Benchmark" "Compile" "Run(ms)" "Binary" "Exit"
printf "%-25s %10s %10s %8s %6s\n" "---------" "-------" "-------" "------" "----"

PASS=0
FAIL=0

for bench in "$BENCH_DIR"/*.ori; do
    name=$(basename "$bench" .ori)
    out="$TMPDIR/$name"

    # Compile
    compile_start=$(date +%s%N)
    if ! "$ORI" build "$bench" -o "$out" 2>/dev/null; then
        printf "%-25s %10s %10s %8s %6s\n" "$name" "FAIL" "-" "-" "ERR"
        FAIL=$((FAIL + 1))
        continue
    fi
    compile_end=$(date +%s%N)
    compile_ms=$(( (compile_end - compile_start) / 1000000 ))

    # Binary size
    bin_size=$(stat -c%s "$out" 2>/dev/null || echo "0")
    bin_kb=$((bin_size / 1024))

    # Run (median of 3)
    times=()
    exit_code=0
    for i in 1 2 3; do
        run_start=$(date +%s%N)
        "$out" || exit_code=$?
        run_end=$(date +%s%N)
        run_ms=$(( (run_end - run_start) / 1000000 ))
        times+=("$run_ms")
    done

    # Sort and take median
    IFS=$'\n' sorted=($(sort -n <<<"${times[*]}")); unset IFS
    median=${sorted[1]}

    status="OK"
    if [[ $exit_code -ne 0 ]]; then
        status="FAIL"
        FAIL=$((FAIL + 1))
    else
        PASS=$((PASS + 1))
    fi

    printf "%-25s %8dms %8dms %6dK %6s\n" "$name" "$compile_ms" "$median" "$bin_kb" "$status"
done

echo
echo "--- Summary ---"
echo "Pass: $PASS  Fail: $FAIL  Total: $((PASS + FAIL))"
echo "Timestamp: $(date -Iseconds)"
echo "Mode: $MODE"
