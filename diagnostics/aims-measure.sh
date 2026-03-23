#!/usr/bin/env bash
# aims-measure.sh — Capture AIMS pipeline measurements as JSON records.
#
# Records: compile time, runtime, peak RSS, binary size, RC counts per program.
# Also captures hardware/environment context for cross-machine comparability.
#
# Usage:
#   diagnostics/aims-measure.sh [options] [test-path]
#
# Options:
#   --release      Use release binary (default: debug)
#   --save FILE    Save results to JSON file (default: build/aims-history/<commit>.json)
#   --compare FILE Compare against a baseline JSON, flag regressions
#   --no-color     Disable color output
#   --color        Force color output
#   -h, --help     Show this help
#
# Example:
#   diagnostics/aims-measure.sh --release --save build/aims-history/baseline.json
#   diagnostics/aims-measure.sh --release --compare build/aims-history/baseline.json

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# Defaults
TEST_PATH="tests/benchmarks/"
BUILD_PROFILE="debug"
BUILD_FLAGS=""
SAVE_FILE=""
COMPARE_FILE=""
USE_COLOR=auto
REGRESSION_THRESHOLD=10  # percent

while [[ $# -gt 0 ]]; do
    case $1 in
        --release) BUILD_PROFILE="release"; BUILD_FLAGS="--release"; shift ;;
        --save) SAVE_FILE="$2"; shift 2 ;;
        --compare) COMPARE_FILE="$2"; shift 2 ;;
        --color) USE_COLOR=yes; shift ;;
        --no-color) USE_COLOR=no; shift ;;
        -h|--help)
            sed -n '2,/^$/{ s/^# \?//; p }' "$0"
            exit 0
            ;;
        -*)
            echo "Error: unknown option: $1" >&2
            echo "Run with --help for usage." >&2
            exit 2
            ;;
        *)
            if [[ -e "$1" || -e "$ROOT_DIR/$1" ]]; then
                TEST_PATH="$1"
            else
                echo "Error: path not found: $1" >&2
                exit 2
            fi
            shift
            ;;
    esac
done

# Color codes
if [[ "$USE_COLOR" == "auto" ]]; then
    [[ -t 1 ]] && USE_COLOR=yes || USE_COLOR=no
fi
if [[ "$USE_COLOR" == "yes" ]]; then
    C_RED='\033[0;31m'
    C_GREEN='\033[0;32m'
    C_YELLOW='\033[0;33m'
    C_BOLD='\033[1m'
    C_DIM='\033[2m'
    C_NC='\033[0m'
else
    C_RED="" C_GREEN="" C_YELLOW="" C_BOLD="" C_DIM="" C_NC=""
fi

ORI_BIN="$ROOT_DIR/target/$BUILD_PROFILE/ori"
HISTORY_DIR="$ROOT_DIR/build/aims-history"
mkdir -p "$HISTORY_DIR"

# Resolve test path
if [[ "$TEST_PATH" != /* ]]; then
    TEST_PATH="$ROOT_DIR/$TEST_PATH"
fi

# Default save file: build/aims-history/<commit>.json
if [[ -z "$SAVE_FILE" ]]; then
    COMMIT=$(git -C "$ROOT_DIR" rev-parse --short HEAD 2>/dev/null || echo "unknown")
    SAVE_FILE="$HISTORY_DIR/${COMMIT}-$(date +%Y%m%d-%H%M%S).json"
fi

# Hardware/environment context (E5)
collect_context() {
    local hostname cpu_model memory_kb os_info rust_ver llvm_ver commit profile
    hostname=$(hostname 2>/dev/null || echo "unknown")
    cpu_model=$(grep -m1 'model name' /proc/cpuinfo 2>/dev/null | cut -d: -f2 | xargs || echo "unknown")
    memory_kb=$(grep MemTotal /proc/meminfo 2>/dev/null | awk '{print $2}' || echo "0")
    os_info=$(uname -srm 2>/dev/null || echo "unknown")
    rust_ver=$(rustc --version 2>/dev/null || echo "unknown")
    llvm_ver=$(llvm-config --version 2>/dev/null || echo "unknown")
    commit=$(git -C "$ROOT_DIR" rev-parse HEAD 2>/dev/null || echo "unknown")
    profile="$BUILD_PROFILE"

    cat <<CTXEOF
    "context": {
      "hostname": "$hostname",
      "cpu": "$cpu_model",
      "memory_mb": $((memory_kb / 1024)),
      "os": "$os_info",
      "rust": "$rust_ver",
      "llvm": "$llvm_ver",
      "commit": "$commit",
      "profile": "$profile",
      "timestamp": "$(date -Iseconds)"
    }
CTXEOF
}

# Peak RSS measurement (E6) via /usr/bin/time -v (Linux)
measure_peak_rss() {
    local binary="$1"
    local time_output
    time_output=$(/usr/bin/time -v "$binary" 2>&1 >/dev/null || true)
    echo "$time_output" | grep "Maximum resident set size" | awk '{print $NF}' || echo "0"
}

# Build the compiler
echo -e "${C_BOLD}Building compiler ($BUILD_PROFILE)...${C_NC}"
if ! cargo build -p oric $BUILD_FLAGS 2>&1 | tail -1; then
    echo "Error: build failed" >&2
    exit 2
fi

# Find @main programs
find_main_programs() {
    local search_path="$1"
    if [[ -f "$search_path" ]]; then
        grep -l '@main' "$search_path" 2>/dev/null || true
    else
        grep -rl '@main' "$search_path" --include="*.ori" 2>/dev/null | sort || true
    fi
}

PROGRAMS=$(find_main_programs "$TEST_PATH")
PROGRAM_COUNT=$(echo "$PROGRAMS" | grep -c . || echo 0)

echo -e "${C_BOLD}Measuring $PROGRAM_COUNT programs from $TEST_PATH...${C_NC}"
echo ""

# JSON array of measurements
JSON_ENTRIES=""
SEP=""

TMP_BIN=$(mktemp)
trap "rm -f $TMP_BIN" EXIT

while IFS= read -r program; do
    [[ -z "$program" ]] && continue
    rel_path="${program#$ROOT_DIR/}"
    name=$(basename "$program" .ori)

    # Compile time (average of 3 runs)
    total_compile_ms=0
    for _ in 1 2 3; do
        start_ns=$(date +%s%N)
        "$ORI_BIN" build "$program" -o "$TMP_BIN" 2>/dev/null || continue
        end_ns=$(date +%s%N)
        elapsed_ms=$(( (end_ns - start_ns) / 1000000 ))
        total_compile_ms=$((total_compile_ms + elapsed_ms))
    done
    compile_ms=$((total_compile_ms / 3))

    # Binary size
    binary_size=0
    if [[ -f "$TMP_BIN" ]]; then
        binary_size=$(stat -c%s "$TMP_BIN" 2>/dev/null || echo 0)
    fi

    # Runtime (median of 3 runs) + peak RSS
    run_times=()
    peak_rss_kb=0
    for _ in 1 2 3; do
        if [[ -x "$TMP_BIN" ]]; then
            start_ns=$(date +%s%N)
            "$TMP_BIN" >/dev/null 2>&1 || true
            end_ns=$(date +%s%N)
            elapsed_ms=$(( (end_ns - start_ns) / 1000000 ))
            run_times+=("$elapsed_ms")
        fi
    done

    # Median of 3
    run_ms=0
    if [[ ${#run_times[@]} -eq 3 ]]; then
        IFS=$'\n' sorted=($(sort -n <<<"${run_times[*]}")); unset IFS
        run_ms="${sorted[1]}"
    fi

    # Peak RSS (single measurement)
    if [[ -x "$TMP_BIN" ]]; then
        peak_rss_kb=$(measure_peak_rss "$TMP_BIN")
    fi

    # RC counts from ARC IR dump
    rc_count=0
    arc_dump=$(ORI_DUMP_AFTER_ARC=1 "$ORI_BIN" build "$program" -o /dev/null 2>&1 || true)
    if [[ -n "$arc_dump" ]]; then
        inc=$(echo "$arc_dump" | grep -c "RcInc" || true)
        dec=$(echo "$arc_dump" | grep -c "RcDec" || true)
        rc_count=$((inc + dec))
    fi

    printf "  %-30s compile=%dms  run=%dms  rss=%dKB  rc=%d  binary=%d\n" \
        "$name" "$compile_ms" "$run_ms" "$peak_rss_kb" "$rc_count" "$binary_size"

    JSON_ENTRIES="${JSON_ENTRIES}${SEP}
    {
      \"name\": \"$name\",
      \"path\": \"$rel_path\",
      \"compile_ms\": $compile_ms,
      \"run_ms\": $run_ms,
      \"peak_rss_kb\": $peak_rss_kb,
      \"rc_ops\": $rc_count,
      \"binary_bytes\": $binary_size
    }"
    SEP=","
done <<< "$PROGRAMS"

echo ""

# Write JSON record
CONTEXT=$(collect_context)
cat > "$SAVE_FILE" <<JSONEOF
{
$CONTEXT,
  "programs": [$JSON_ENTRIES
  ]
}
JSONEOF

echo -e "${C_GREEN}Saved to: $SAVE_FILE${C_NC}"

# Baseline comparison (E7)
if [[ -n "$COMPARE_FILE" ]]; then
    if [[ ! -f "$COMPARE_FILE" ]]; then
        echo -e "${C_RED}Error: baseline file not found: $COMPARE_FILE${C_NC}" >&2
        exit 2
    fi

    echo ""
    echo -e "${C_BOLD}=== Baseline Comparison ===${C_NC}"
    echo -e "${C_DIM}Baseline: $COMPARE_FILE${C_NC}"
    echo -e "${C_DIM}Regression threshold: ${REGRESSION_THRESHOLD}%${C_NC}"
    echo ""

    regressions=0
    improvements=0

    # Parse baseline programs (simple jq-free parsing)
    while IFS= read -r program; do
        [[ -z "$program" ]] && continue
        name=$(basename "$program" .ori)

        # Extract current measurement
        current_run=$(grep -A5 "\"name\": \"$name\"" "$SAVE_FILE" | grep run_ms | head -1 | grep -oP '\d+' || echo "0")
        current_compile=$(grep -A5 "\"name\": \"$name\"" "$SAVE_FILE" | grep compile_ms | head -1 | grep -oP '\d+' || echo "0")

        # Extract baseline measurement
        baseline_run=$(grep -A5 "\"name\": \"$name\"" "$COMPARE_FILE" | grep run_ms | head -1 | grep -oP '\d+' || echo "0")
        baseline_compile=$(grep -A5 "\"name\": \"$name\"" "$COMPARE_FILE" | grep compile_ms | head -1 | grep -oP '\d+' || echo "0")

        if [[ "$baseline_run" -gt 0 && "$current_run" -gt 0 ]]; then
            delta_pct=$(( (current_run - baseline_run) * 100 / baseline_run ))
            if [[ "$delta_pct" -gt "$REGRESSION_THRESHOLD" ]]; then
                echo -e "  ${C_RED}REGRESSED${C_NC} $name: run ${baseline_run}ms → ${current_run}ms (+${delta_pct}%)"
                regressions=$((regressions + 1))
            elif [[ "$delta_pct" -lt "-$REGRESSION_THRESHOLD" ]]; then
                echo -e "  ${C_GREEN}IMPROVED${C_NC}  $name: run ${baseline_run}ms → ${current_run}ms (${delta_pct}%)"
                improvements=$((improvements + 1))
            fi
        fi
    done <<< "$PROGRAMS"

    echo ""
    echo -e "${C_BOLD}Regressions: $regressions  Improvements: $improvements${C_NC}"

    if [[ "$regressions" -gt 0 ]]; then
        exit 1
    fi
fi
