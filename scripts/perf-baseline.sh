#!/usr/bin/env bash
# Performance Baseline Script
#
# Measures compile time, runtime (AOT vs JIT), and binary sizes
# for a set of benchmark programs. Results are printed in a
# machine-readable table format suitable for tracking over time.
#
# Usage: ./scripts/perf-baseline.sh [--release] [--include-cow]
#
# Options:
#   --release       Use release binary (default: debug)
#   --include-cow   Include COW benchmark suite (cow-benchmark.sh)
#
# Requires: the LLVM-enabled ori binary in target/debug/ori
#           (or target/release/ori with --release)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
TMPDIR="${ROOT_DIR}/build/perf-baseline"
mkdir -p "$TMPDIR"

# Parse arguments
ORI="${ROOT_DIR}/target/debug/ori"
MODE="debug"
INCLUDE_COW=false
for arg in "$@"; do
    case "$arg" in
        --release)
            ORI="${ROOT_DIR}/target/release/ori"
            MODE="release" ;;
        --include-cow)
            INCLUDE_COW=true ;;
    esac
done

if [[ ! -x "$ORI" ]]; then
    echo "ERROR: $ORI not found. Run 'cargo b' (or 'cargo b --release' for release) first."
    exit 1
fi

# Benchmark programs
BENCH_HELLO="${ROOT_DIR}/tests/benchmarks/bench_hello.ori"
BENCH_SMALL="${ROOT_DIR}/tests/benchmarks/bench_small.ori"
BENCH_MEDIUM="${ROOT_DIR}/tests/benchmarks/bench_medium.ori"

# Create minimal hello world if missing
if [[ ! -f "$BENCH_HELLO" ]]; then
    printf '@main () -> int = 0;\n' > "$BENCH_HELLO"
fi

echo "============================================"
echo "  Ori Performance Baseline ($MODE)"
echo "  Date: $(date -Iseconds)"
echo "  Binary: $ORI"
echo "============================================"
echo

# --- Compile Time ---
echo "--- Compile Time (ori build) ---"
printf "%-20s %10s %10s %8s\n" "Program" "Lines" "Time(ms)" "Binary"
printf "%-20s %10s %10s %8s\n" "--------" "-----" "--------" "------"

for bench in "$BENCH_HELLO" "$BENCH_SMALL" "$BENCH_MEDIUM"; do
    name=$(basename "$bench" .ori)
    lines=$(wc -l < "$bench")
    out="$TMPDIR/$name"

    # Measure compile time (average of 3 runs)
    total_ms=0
    for i in 1 2 3; do
        start_ns=$(date +%s%N)
        "$ORI" build "$bench" -o "$out" 2>/dev/null
        end_ns=$(date +%s%N)
        elapsed_ms=$(( (end_ns - start_ns) / 1000000 ))
        total_ms=$((total_ms + elapsed_ms))
    done
    avg_ms=$((total_ms / 3))

    # Binary size
    bin_size=$(stat -c%s "$out" 2>/dev/null || echo "0")
    bin_kb=$((bin_size / 1024))

    printf "%-20s %10d %10d %7dK\n" "$name" "$lines" "$avg_ms" "$bin_kb"
done

echo

# --- Runtime Performance (AOT vs JIT) ---
echo "--- Runtime Performance (AOT vs JIT) ---"
printf "%-20s %10s %10s %10s\n" "Program" "JIT(ms)" "AOT(ms)" "Speedup"
printf "%-20s %10s %10s %10s\n" "--------" "-------" "-------" "-------"

for bench in "$BENCH_SMALL" "$BENCH_MEDIUM"; do
    name=$(basename "$bench" .ori)
    out="$TMPDIR/$name"

    # Build AOT binary
    "$ORI" build "$bench" -o "$out" 2>/dev/null

    # JIT: average of 3 runs
    jit_total=0
    for i in 1 2 3; do
        start_ns=$(date +%s%N)
        "$ORI" run "$bench" 2>/dev/null
        end_ns=$(date +%s%N)
        elapsed_ms=$(( (end_ns - start_ns) / 1000000 ))
        jit_total=$((jit_total + elapsed_ms))
    done
    jit_avg=$((jit_total / 3))

    # AOT: average of 3 runs
    aot_total=0
    for i in 1 2 3; do
        start_ns=$(date +%s%N)
        "$out"
        end_ns=$(date +%s%N)
        elapsed_ms=$(( (end_ns - start_ns) / 1000000 ))
        aot_total=$((aot_total + elapsed_ms))
    done
    aot_avg=$((aot_total / 3))

    if [[ $aot_avg -gt 0 ]]; then
        speedup=$(echo "scale=1; $jit_avg / $aot_avg" | bc)
    else
        speedup="inf"
    fi

    printf "%-20s %10d %10d %9sx\n" "$name" "$jit_avg" "$aot_avg" "$speedup"
done

echo

# --- Binary Size ---
echo "--- Binary Size ---"
printf "%-20s %10s %10s\n" "Program" "Size" "Stripped"
printf "%-20s %10s %10s\n" "--------" "----" "--------"

for bench in "$BENCH_HELLO" "$BENCH_SMALL" "$BENCH_MEDIUM"; do
    name=$(basename "$bench" .ori)
    out="$TMPDIR/$name"
    stripped="$TMPDIR/${name}_stripped"

    "$ORI" build "$bench" -o "$out" 2>/dev/null

    bin_size=$(stat -c%s "$out" 2>/dev/null || echo "0")
    bin_kb=$((bin_size / 1024))

    # Create stripped copy
    cp "$out" "$stripped"
    strip "$stripped" 2>/dev/null || true
    stripped_size=$(stat -c%s "$stripped" 2>/dev/null || echo "0")
    stripped_kb=$((stripped_size / 1024))

    printf "%-20s %9dK %9dK\n" "$name" "$bin_kb" "$stripped_kb"
done

# --- COW Benchmarks (optional) ---
if [[ "$INCLUDE_COW" == "true" ]]; then
    echo
    echo "--- COW Benchmark Suite ---"
    COW_SCRIPT="${SCRIPT_DIR}/cow-benchmark.sh"
    if [[ -x "$COW_SCRIPT" ]]; then
        COW_ARGS="--include-macro"
        if [[ "$MODE" == "release" ]]; then
            COW_ARGS="--release $COW_ARGS"
        fi
        BASELINE="${ROOT_DIR}/tests/benchmarks/cow/baseline.json"
        if [[ -f "$BASELINE" ]]; then
            COW_ARGS="$COW_ARGS --compare $BASELINE"
        fi
        "$COW_SCRIPT" $COW_ARGS || true
    else
        echo "WARNING: cow-benchmark.sh not found at $COW_SCRIPT"
    fi
fi

echo
echo "--- Summary ---"
echo "Timestamp: $(date -Iseconds)"
echo "Compiler: $($ORI --version 2>&1 | head -1)"
echo "Mode: $MODE"
echo "Platform: $(uname -m) $(uname -s)"
