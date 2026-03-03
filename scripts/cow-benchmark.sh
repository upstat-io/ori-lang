#!/usr/bin/env bash
# COW Benchmark Runner
#
# Compiles and runs all COW benchmark programs, reporting times.
# Measures both compile time and runtime for each benchmark.
#
# Usage: ./scripts/cow-benchmark.sh [OPTIONS]
#
# Options:
#   --release       Use release binary (default: debug)
#   --json          Output results as JSON (to stdout)
#   --compare FILE  Compare results against baseline JSON, flag regressions >10%
#   --save FILE     Save results as baseline JSON
#   --include-macro Include macro benchmarks from cow/macro/
#   --help          Show this help
#
# Requires: the LLVM-enabled ori binary

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
BENCH_DIR="${ROOT_DIR}/tests/benchmarks/cow"
TMPDIR="${ROOT_DIR}/build/cow-benchmarks"
BASELINE_PATH="${BENCH_DIR}/baseline.json"
mkdir -p "$TMPDIR"

# Parse arguments
ORI="${ROOT_DIR}/target/debug/ori"
MODE="debug"
JSON_OUTPUT=false
COMPARE_FILE=""
SAVE_FILE=""
INCLUDE_MACRO=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        --release)
            ORI="${ROOT_DIR}/target/release/ori"
            MODE="release"
            shift ;;
        --json)
            JSON_OUTPUT=true
            shift ;;
        --compare)
            COMPARE_FILE="${2:-$BASELINE_PATH}"
            shift; shift ;;
        --save)
            SAVE_FILE="${2:-$BASELINE_PATH}"
            shift; shift ;;
        --include-macro)
            INCLUDE_MACRO=true
            shift ;;
        --help)
            head -17 "$0" | tail -14
            exit 0 ;;
        *)
            echo "Unknown option: $1 (try --help)"
            exit 1 ;;
    esac
done

if [[ ! -x "$ORI" ]]; then
    echo "ERROR: $ORI not found. Run 'cargo b' (or 'cargo b --release' for release) first." >&2
    exit 1
fi

if [[ ! -d "$BENCH_DIR" ]]; then
    echo "ERROR: $BENCH_DIR not found." >&2
    exit 1
fi

# Collect benchmark files
bench_files=("$BENCH_DIR"/*.ori)
if [[ "$INCLUDE_MACRO" == "true" && -d "$BENCH_DIR/macro" ]]; then
    bench_files+=("$BENCH_DIR"/macro/*.ori)
fi

# Load baseline for comparison
declare -A BASELINE_RUN=()
if [[ -n "$COMPARE_FILE" && -f "$COMPARE_FILE" ]]; then
    # Parse baseline JSON into associative array (name -> run_ms)
    while IFS='=' read -r key val; do
        BASELINE_RUN["$key"]="$val"
    done < <(python3 -c "
import json, sys
with open('$COMPARE_FILE') as f:
    data = json.load(f)
for name, info in data.items():
    print(f\"{name}={info['run_ms']}\")
" 2>/dev/null || true)
fi

# Header (unless JSON mode)
if [[ "$JSON_OUTPUT" != "true" ]]; then
    echo "============================================"
    echo "  COW Benchmark Suite ($MODE)"
    echo "  Date: $(date -Iseconds)"
    echo "  Binary: $ORI"
    echo "============================================"
    echo

    if [[ -n "$COMPARE_FILE" ]]; then
        header_fmt="%-25s %10s %10s %10s %8s %6s\n"
        printf "$header_fmt" "Benchmark" "Compile" "Run(ms)" "Baseline" "Binary" "Exit"
        printf "$header_fmt" "---------" "-------" "-------" "--------" "------" "----"
    else
        printf "%-25s %10s %10s %8s %6s\n" "Benchmark" "Compile" "Run(ms)" "Binary" "Exit"
        printf "%-25s %10s %10s %8s %6s\n" "---------" "-------" "-------" "------" "----"
    fi
fi

PASS=0
FAIL=0
REGRESSIONS=0
JSON_ENTRIES=""

for bench in "${bench_files[@]}"; do
    # Compute name: "macro/foo" for macro benchmarks, "foo" for micro
    rel_path="${bench#"$BENCH_DIR"/}"
    name="${rel_path%.ori}"

    out="$TMPDIR/${name//\//_}"

    # Compile
    compile_start=$(date +%s%N)
    if ! "$ORI" build "$bench" -o "$out" 2>/dev/null; then
        if [[ "$JSON_OUTPUT" != "true" ]]; then
            printf "%-25s %10s %10s %8s %6s\n" "$name" "FAIL" "-" "-" "ERR"
        fi
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
        "$out" 2>/dev/null || exit_code=$?
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

    # Regression check
    regression_note=""
    if [[ -n "$COMPARE_FILE" && "${BASELINE_RUN[$name]+_}" && "$status" == "OK" ]]; then
        baseline_ms="${BASELINE_RUN[$name]}"
        if [[ "$baseline_ms" -gt 0 ]]; then
            # Check for >10% regression
            threshold=$(( baseline_ms + baseline_ms / 10 ))
            if [[ "$median" -gt "$threshold" ]]; then
                regression_note="REGRESSED (+$(( (median - baseline_ms) * 100 / baseline_ms ))%)"
                REGRESSIONS=$((REGRESSIONS + 1))
            else
                regression_note="${baseline_ms}ms"
            fi
        else
            regression_note="${baseline_ms}ms"
        fi
    fi

    if [[ "$JSON_OUTPUT" != "true" ]]; then
        if [[ -n "$COMPARE_FILE" ]]; then
            printf "%-25s %8dms %8dms %10s %6dK %6s\n" "$name" "$compile_ms" "$median" "$regression_note" "$bin_kb" "$status"
        else
            printf "%-25s %8dms %8dms %6dK %6s\n" "$name" "$compile_ms" "$median" "$bin_kb" "$status"
        fi
    fi

    # Accumulate JSON entries
    if [[ -n "$JSON_ENTRIES" ]]; then
        JSON_ENTRIES="${JSON_ENTRIES},"
    fi
    JSON_ENTRIES="${JSON_ENTRIES}
    \"${name}\": {\"compile_ms\": ${compile_ms}, \"run_ms\": ${median}, \"binary_kb\": ${bin_kb}, \"status\": \"${status}\"}"
done

# JSON output
JSON_BLOB="{
    \"timestamp\": \"$(date -Iseconds)\",
    \"mode\": \"${MODE}\",
    \"results\": {${JSON_ENTRIES}
    }
}"

if [[ "$JSON_OUTPUT" == "true" ]]; then
    echo "$JSON_BLOB"
fi

# Save baseline
if [[ -n "$SAVE_FILE" ]]; then
    # Extract just the results object for baseline (simpler format)
    python3 -c "
import json, sys
data = json.loads('''${JSON_BLOB}''')
baseline = {}
for name, info in data['results'].items():
    baseline[name] = {'run_ms': info['run_ms'], 'compile_ms': info['compile_ms'], 'binary_kb': info['binary_kb']}
with open('$SAVE_FILE', 'w') as f:
    json.dump(baseline, f, indent=4, sort_keys=True)
print(f'Baseline saved to $SAVE_FILE ({len(baseline)} benchmarks)', file=sys.stderr)
"
fi

# Summary (unless JSON mode)
if [[ "$JSON_OUTPUT" != "true" ]]; then
    echo
    echo "--- Summary ---"
    echo "Pass: $PASS  Fail: $FAIL  Total: $((PASS + FAIL))"
    if [[ -n "$COMPARE_FILE" ]]; then
        if [[ $REGRESSIONS -gt 0 ]]; then
            echo "REGRESSIONS: $REGRESSIONS benchmarks regressed >10% from baseline"
        else
            echo "No regressions detected (all within 10% of baseline)"
        fi
    fi
    echo "Timestamp: $(date -Iseconds)"
    echo "Mode: $MODE"
fi

# Exit with non-zero if any regressions detected (for CI)
if [[ $REGRESSIONS -gt 0 ]]; then
    exit 1
fi
