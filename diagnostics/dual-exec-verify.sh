#!/bin/bash
# Dual-Execution Verification: Compare interpreter vs LLVM backend results.
#
# Usage:
#   diagnostics/dual-exec-verify.sh [options] [test-path]
#
# Options:
#   -v, --verbose      Show all test results, not just mismatches
#   --json[=PATH]      Emit JSON report (default: build/dual-exec-report.json)
#   --main-only        Only run @main program comparison (skip @test comparison)
#   --test-only        Only run @test comparison (skip @main programs)
#   --no-color         Disable color output
#   --color            Force color output (default: auto-detect terminal)
#   -h, --help         Show this help
#
# Runs all spec tests through both backends and cross-references results
# to detect behavioral mismatches.
#
# Environment:
#   ORI_BIN    Override path to ori binary (used for both interpreter and LLVM
#              if the binary has LLVM support; interpreter-only otherwise)
#
# Exit codes:
#   0 = No behavioral mismatches detected (with at least one verification)
#   1 = Behavioral mismatches found (PASS in one backend, FAIL in other)
#   2 = Infrastructure error (build failure, binary not found)
#   3 = Zero verifications (no tests were actually compared — misleading "all clear")

set -uo pipefail
# Note: NOT using set -e because functions return mismatch counts as exit codes

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
source "$SCRIPT_DIR/_common.sh"

# --- Defaults ---
TEST_PATH="tests/"
INTERP_BIN=""
LLVM_BIN=""
VERBOSE=0
EMIT_JSON=0
JSON_PATH="$ROOT_DIR/build/dual-exec-report.json"
RUN_TESTS=1
RUN_MAIN=1
USE_COLOR=auto
PER_TEST_TIMEOUT=10  # seconds per individual test/build/run (SIGKILL at +5s)

# --- Parse arguments ---
while [[ $# -gt 0 ]]; do
    case $1 in
        -v|--verbose) VERBOSE=1; shift ;;
        --json) EMIT_JSON=1; shift ;;
        --json=*) EMIT_JSON=1; JSON_PATH="${1#--json=}"; shift ;;
        --main-only) RUN_TESTS=0; shift ;;
        --test-only) RUN_MAIN=0; shift ;;
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

# --- Locate binaries ---
# Interpreter: any ori binary (no LLVM needed)
find_any_ori_bin
INTERP_BIN="$ORI_INTERP"

# LLVM: needs LLVM support
find_ori_bin
LLVM_BIN="$ORI"

# --- Resolve color mode ---
# (placed after binary lookup to keep error ordering predictable)
if [[ "$USE_COLOR" == "auto" ]]; then
    if [[ -t 1 ]]; then
        USE_COLOR=yes
    else
        USE_COLOR=no
    fi
fi

# --- Color codes ---
if [[ "$USE_COLOR" == "yes" ]]; then
    C_RED='\033[0;31m'
    C_GREEN='\033[0;32m'
    C_YELLOW='\033[0;33m'
    C_CYAN='\033[0;36m'
    C_BOLD='\033[1m'
    C_DIM='\033[2m'
    C_NC='\033[0m'
else
    C_RED="" C_GREEN="" C_YELLOW="" C_CYAN="" C_BOLD="" C_DIM="" C_NC=""
fi

# --- Temp files ---
INTERP_OUTPUT=$(mktemp)
LLVM_OUTPUT=$(mktemp)
cleanup() { rm -f "$INTERP_OUTPUT" "$LLVM_OUTPUT"; }
trap cleanup EXIT

# =============================================================================
# Part 1: @test function comparison (interpreter vs LLVM JIT)
# =============================================================================

# Counters
VERIFIED=0          # PASS in both
INTERP_ONLY=0       # PASS in interpreter, LLVM compile fail
LLVM_BLOCKED=0      # Test blocked by file-level LLVM compile failure
MISMATCH_INTERP=0   # PASS in interpreter, FAIL in LLVM
MISMATCH_LLVM=0     # FAIL in interpreter, PASS in LLVM
BOTH_SKIP=0         # SKIP in both
BOTH_FAIL=0         # FAIL in both

# Arrays for mismatch details
declare -a MISMATCH_DETAILS=()

parse_test_results() {
    # Parse verbose test output into a flat file: "FILE\tTEST\tSTATUS"
    # STATUS is one of: PASS, FAIL, SKIP, LCFAIL, BLOCKED
    local input_file="$1"
    local output_file="$2"
    local current_file=""

    > "$output_file"

    while IFS= read -r line; do
        # File header: path at start of line (no leading whitespace)
        if [[ "$line" =~ ^[a-zA-Z./] ]] && [[ "$line" =~ \.ori$ ]]; then
            current_file="$line"
            continue
        fi

        # Blocked tests: "  (N tests blocked)"
        if [[ "$line" =~ ^[[:space:]]*\(([0-9]+)\ tests\ blocked\) ]]; then
            local count="${BASH_REMATCH[1]}"
            for ((i=0; i<count; i++)); do
                echo -e "${current_file}\t__blocked_${i}\tBLOCKED" >> "$output_file"
            done
            continue
        fi

        # Individual test results
        if [[ "$line" =~ ^[[:space:]]*PASS:\ ([^ ]+) ]]; then
            echo -e "${current_file}\t${BASH_REMATCH[1]}\tPASS" >> "$output_file"
        elif [[ "$line" =~ ^[[:space:]]*FAIL:\ ([^ ]+) ]]; then
            echo -e "${current_file}\t${BASH_REMATCH[1]}\tFAIL" >> "$output_file"
        elif [[ "$line" =~ ^[[:space:]]*SKIP:\ ([^ ]+) ]]; then
            echo -e "${current_file}\t${BASH_REMATCH[1]}\tSKIP" >> "$output_file"
        elif [[ "$line" =~ ^[[:space:]]*LLVM\ COMPILE\ FAIL:\ ([^ ]+) ]]; then
            echo -e "${current_file}\t${BASH_REMATCH[1]}\tLCFAIL" >> "$output_file"
        fi
    done < "$input_file"
}

cross_reference() {
    # Cross-reference interpreter and LLVM results
    local interp_parsed="$1"
    local llvm_parsed="$2"

    # Build associative arrays from LLVM results
    declare -A llvm_results=()
    while IFS=$'\t' read -r file test status; do
        # For BLOCKED entries, we track at file level
        if [[ "$status" == "BLOCKED" ]]; then
            llvm_results["${file}:__file_blocked"]="BLOCKED"
        else
            llvm_results["${file}:${test}"]="$status"
        fi
    done < "$llvm_parsed"

    # Walk interpreter results and cross-reference
    while IFS=$'\t' read -r file test status; do
        local key="${file}:${test}"
        local llvm_status="${llvm_results[$key]:-MISSING}"

        # Check if file was blocked in LLVM
        if [[ "$llvm_status" == "MISSING" ]] && [[ "${llvm_results[${file}:__file_blocked]:-}" == "BLOCKED" ]]; then
            llvm_status="BLOCKED"
        fi

        case "${status}:${llvm_status}" in
            PASS:PASS)
                ((VERIFIED++))
                if [[ $VERBOSE -eq 1 ]]; then
                    printf "  ${C_GREEN}VERIFIED${C_NC}: %s :: %s\n" "$file" "$test"
                fi
                ;;
            PASS:BLOCKED|PASS:LCFAIL|PASS:MISSING)
                ((INTERP_ONLY++))
                ;;
            PASS:FAIL)
                ((MISMATCH_INTERP++))
                MISMATCH_DETAILS+=("${C_RED}MISMATCH${C_NC}: $file :: $test — PASS (interp) vs FAIL (LLVM)")
                ;;
            FAIL:PASS)
                ((MISMATCH_LLVM++))
                MISMATCH_DETAILS+=("${C_RED}MISMATCH${C_NC}: $file :: $test — FAIL (interp) vs PASS (LLVM)")
                ;;
            SKIP:SKIP|SKIP:BLOCKED|SKIP:LCFAIL|SKIP:MISSING)
                ((BOTH_SKIP++))
                ;;
            FAIL:FAIL|FAIL:BLOCKED|FAIL:LCFAIL|FAIL:MISSING)
                ((BOTH_FAIL++))
                ;;
            *)
                # Other combinations (e.g., SKIP:PASS) — unusual but not critical
                if [[ $VERBOSE -eq 1 ]]; then
                    printf "  ${C_DIM}OTHER${C_NC}: %s :: %s — %s (interp) vs %s (LLVM)\n" \
                        "$file" "$test" "$status" "$llvm_status"
                fi
                ;;
        esac
    done < "$interp_parsed" || true
}

run_test_comparison() {
    printf "${C_BOLD}=== Dual-Execution Verification: @test functions ===${C_NC}\n\n"

    # Run interpreter
    echo -n "  Running interpreter backend..."
    timeout -k 5 120 env ORI_LOG=off "$INTERP_BIN" test --verbose "$TEST_PATH" > "$INTERP_OUTPUT" 2>&1 || true
    local interp_summary
    interp_summary=$(grep -E "^  [0-9]+ passed" "$INTERP_OUTPUT" | tail -1)
    printf " ${C_GREEN}done${C_NC} (%s)\n" "$interp_summary"

    # Run LLVM
    echo -n "  Running LLVM backend..."
    timeout -k 5 120 env ORI_LOG=off "$LLVM_BIN" test --verbose --backend=llvm "$TEST_PATH" > "$LLVM_OUTPUT" 2>&1 || true
    local llvm_summary
    llvm_summary=$(grep -E "^  [0-9]+ passed" "$LLVM_OUTPUT" | tail -1)
    printf " ${C_GREEN}done${C_NC} (%s)\n\n" "$llvm_summary"

    # Parse results
    local interp_parsed llvm_parsed
    interp_parsed=$(mktemp)
    llvm_parsed=$(mktemp)

    parse_test_results "$INTERP_OUTPUT" "$interp_parsed"
    parse_test_results "$LLVM_OUTPUT" "$llvm_parsed"

    # Cross-reference
    cross_reference "$interp_parsed" "$llvm_parsed"

    # Clean up parsed files
    rm -f "$interp_parsed" "$llvm_parsed"

    # Extract summary numbers from both outputs
    local interp_total llvm_total llvm_lcfail
    interp_total=$(grep -oP '\d+(?= passed)' "$INTERP_OUTPUT" | tail -1)
    llvm_total=$(grep -oP '\d+(?= passed)' "$LLVM_OUTPUT" | tail -1)
    llvm_lcfail=$(grep -oP '\d+(?= llvm compile fail)' "$LLVM_OUTPUT" | tail -1)
    : "${interp_total:=0}" "${llvm_total:=0}" "${llvm_lcfail:=0}"

    # Only count tests that were actually runtime-compared (both PASS with matching output).
    # Previous logic inflated "compile_fail_verified" by counting all non-compared LLVM passes.
    TOTAL_VERIFIED=$((TOTAL_VERIFIED + VERIFIED))
    local total_mismatches=$((MISMATCH_INTERP + MISMATCH_LLVM))

    printf "${C_BOLD}  Results:${C_NC}\n"
    printf "    ${C_GREEN}Verified (runtime, both PASS)${C_NC}:  %d\n" "$VERIFIED"
    printf "    ${C_CYAN}LLVM coverage gap${C_NC}:              %d\n" "$INTERP_ONLY"
    printf "    ${C_DIM}Both skip${C_NC}:                      %d\n" "$BOTH_SKIP"
    if [[ $BOTH_FAIL -gt 0 ]]; then
        printf "    ${C_YELLOW}Both fail${C_NC}:                      %d\n" "$BOTH_FAIL"
    fi

    if [[ $total_mismatches -gt 0 ]]; then
        echo ""
        printf "    ${C_RED}${C_BOLD}BEHAVIORAL MISMATCHES: %d${C_NC}\n\n" "$total_mismatches"
        for detail in "${MISMATCH_DETAILS[@]}"; do
            printf "    %b\n" "$detail"
        done
    else
        echo ""
        printf "    ${C_GREEN}${C_BOLD}No behavioral mismatches detected${C_NC}\n"
    fi

    echo ""
    printf "  ${C_BOLD}Interpreter${C_NC}: %s tests | ${C_BOLD}LLVM${C_NC}: %s pass + %s compile-fail\n" \
        "$interp_total" "$llvm_total" "$llvm_lcfail"
    printf "  ${C_BOLD}Total verified${C_NC}: %d / %s (%d%%)\n\n" \
        "$VERIFIED" "$llvm_total" "$((VERIFIED * 100 / (llvm_total > 0 ? llvm_total : 1)))"

    return $total_mismatches
}

# =============================================================================
# Part 2: @main program comparison (interpreter vs AOT native)
# =============================================================================

MAIN_VERIFIED=0
MAIN_MISMATCH=0
MAIN_AOT_FAIL=0
MAIN_INTERP_FAIL=0
declare -a MAIN_MISMATCH_DETAILS=()

run_main_comparison() {
    printf "${C_BOLD}=== Dual-Execution Verification: @main programs ===${C_NC}\n\n"

    # Find .ori files with @main
    local main_files
    main_files=$(grep -rl '@main' "$ROOT_DIR/$TEST_PATH" 2>/dev/null || true)

    if [[ -z "$main_files" ]]; then
        echo "  No @main programs found in $TEST_PATH"
        echo ""
        return 0
    fi

    local file_count
    file_count=$(echo "$main_files" | wc -l)
    echo "  Found $file_count files with @main"
    echo ""

    local tmp_binary
    tmp_binary=$(mktemp)
    rm -f "$tmp_binary"  # ori build will create it

    while IFS= read -r file; do
        local rel_file="${file#$ROOT_DIR/}"
        printf "  %s ... " "$rel_file"

        # Run interpreter (file-based capture to avoid subshell masking signals)
        local interp_out interp_exit
        local tmp_interp_out
        tmp_interp_out=$(mktemp)
        timeout -k 5 "$PER_TEST_TIMEOUT" env ORI_LOG=off "$INTERP_BIN" run "$file" > "$tmp_interp_out" 2>&1 && interp_exit=0 || interp_exit=$?
        interp_out=$(cat "$tmp_interp_out" 2>/dev/null)
        rm -f "$tmp_interp_out"
        if [[ $interp_exit -eq 124 || $interp_exit -eq 137 ]]; then
            printf "${C_YELLOW}timeout (interp)${C_NC}\n"
            ((MAIN_INTERP_FAIL++))
            continue
        fi

        # Run AOT (file-based capture to avoid subshell masking signals)
        local aot_out aot_exit
        if timeout -k 5 "$PER_TEST_TIMEOUT" env ORI_LOG=off "$LLVM_BIN" build "$file" -o "$tmp_binary" 2>/dev/null; then
            local tmp_aot_out
            tmp_aot_out=$(mktemp)
            { timeout -k 5 "$PER_TEST_TIMEOUT" "$tmp_binary" > "$tmp_aot_out" 2>&1; } 2>/dev/null && aot_exit=0 || aot_exit=$?
            aot_out=$(cat "$tmp_aot_out" 2>/dev/null)
            rm -f "$tmp_aot_out" "$tmp_binary"
            if [[ $aot_exit -eq 124 || $aot_exit -eq 137 ]]; then
                printf "${C_YELLOW}timeout (aot run)${C_NC}\n"
                ((MAIN_AOT_FAIL++))
                continue
            fi
        else
            aot_exit=999  # compile failure
            aot_out="<AOT compile failed>"
        fi

        # Compare
        if [[ $aot_exit -eq 999 ]]; then
            printf "${C_YELLOW}AOT compile fail${C_NC}\n"
            ((MAIN_AOT_FAIL++))
        elif [[ $interp_exit -ne 0 ]] && [[ $aot_exit -ne 0 ]]; then
            printf "${C_DIM}both fail (interp=%d, aot=%d)${C_NC}\n" "$interp_exit" "$aot_exit"
            ((MAIN_INTERP_FAIL++))
        elif [[ "$interp_out" == "$aot_out" ]] && [[ $interp_exit -eq $aot_exit ]]; then
            printf "${C_GREEN}verified${C_NC}\n"
            ((MAIN_VERIFIED++))
        else
            printf "${C_RED}MISMATCH${C_NC}\n"
            ((MAIN_MISMATCH++))
            MAIN_MISMATCH_DETAILS+=("$rel_file")
            if [[ "$interp_out" != "$aot_out" ]]; then
                MAIN_MISMATCH_DETAILS+=("  stdout differs:")
                MAIN_MISMATCH_DETAILS+=("    interp: $(echo "$interp_out" | head -3)")
                MAIN_MISMATCH_DETAILS+=("    aot:    $(echo "$aot_out" | head -3)")
            fi
            if [[ $interp_exit -ne $aot_exit ]]; then
                MAIN_MISMATCH_DETAILS+=("  exit code: interp=$interp_exit, aot=$aot_exit")
            fi
        fi
    done <<< "$main_files" || true

    echo ""
    printf "${C_BOLD}  Results:${C_NC}\n"
    printf "    ${C_GREEN}Verified${C_NC}:         %d\n" "$MAIN_VERIFIED"
    printf "    ${C_YELLOW}AOT compile fail${C_NC}: %d\n" "$MAIN_AOT_FAIL"
    if [[ $MAIN_INTERP_FAIL -gt 0 ]]; then
        printf "    ${C_DIM}Both fail${C_NC}:         %d\n" "$MAIN_INTERP_FAIL"
    fi

    if [[ $MAIN_MISMATCH -gt 0 ]]; then
        echo ""
        printf "    ${C_RED}${C_BOLD}BEHAVIORAL MISMATCHES: %d${C_NC}\n" "$MAIN_MISMATCH"
        for detail in "${MAIN_MISMATCH_DETAILS[@]}"; do
            printf "    %s\n" "$detail"
        done
    else
        echo ""
        printf "    ${C_GREEN}${C_BOLD}No behavioral mismatches detected${C_NC}\n"
    fi
    echo ""

    rm -f "$tmp_binary"
    return $MAIN_MISMATCH
}

# =============================================================================
# Part 3: JSON report
# =============================================================================

emit_json_report() {
    local path="$1"
    local total_mismatches=$((MISMATCH_INTERP + MISMATCH_LLVM + MAIN_MISMATCH))
    local overall="pass"
    if [[ $total_mismatches -gt 0 ]]; then
        overall="fail"
    fi

    mkdir -p "$(dirname "$path")"

    cat > "$path" <<JSONEOF
{
  "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "overall": "$overall",
  "test_comparison": {
    "verified": $VERIFIED,
    "llvm_coverage_gap": $INTERP_ONLY,
    "both_skip": $BOTH_SKIP,
    "both_fail": $BOTH_FAIL,
    "mismatch_interp_pass_llvm_fail": $MISMATCH_INTERP,
    "mismatch_interp_fail_llvm_pass": $MISMATCH_LLVM
  },
  "main_comparison": {
    "verified": $MAIN_VERIFIED,
    "aot_compile_fail": $MAIN_AOT_FAIL,
    "interp_fail": $MAIN_INTERP_FAIL,
    "mismatch": $MAIN_MISMATCH
  }
}
JSONEOF
    echo "JSON report written to $path"
}

# =============================================================================
# Main
# =============================================================================

echo ""
printf "${C_BOLD}Ori Dual-Execution Verification${C_NC}\n"
printf "${C_DIM}Comparing interpreter vs LLVM backend for behavioral equivalence${C_NC}\n\n"

TEST_MISMATCHES=0
MAIN_MISMATCHES=0
TOTAL_VERIFIED=0

if [[ $RUN_TESTS -eq 1 ]]; then
    run_test_comparison || TEST_MISMATCHES=$?
fi

if [[ $RUN_MAIN -eq 1 ]]; then
    run_main_comparison || MAIN_MISMATCHES=$?
fi

# Final summary
TOTAL_MISMATCHES=$((TEST_MISMATCHES + MAIN_MISMATCHES))
TOTAL_VERIFIED=$((TOTAL_VERIFIED + MAIN_VERIFIED))

echo "=============================================="
if [[ $TOTAL_MISMATCHES -gt 0 ]]; then
    printf "${C_RED}${C_BOLD}  DUAL-EXECUTION: %d MISMATCHES FOUND${C_NC}\n" "$TOTAL_MISMATCHES"
elif [[ $TOTAL_VERIFIED -eq 0 ]]; then
    printf "${C_YELLOW}${C_BOLD}  DUAL-EXECUTION: WARNING — ZERO VERIFICATIONS${C_NC}\n"
    printf "${C_YELLOW}  No tests were actually compared across backends.${C_NC}\n"
else
    printf "${C_GREEN}${C_BOLD}  DUAL-EXECUTION: ALL VERIFIED (%d tests)${C_NC}\n" "$TOTAL_VERIFIED"
fi
echo "=============================================="
echo ""

if [[ $EMIT_JSON -eq 1 ]]; then
    emit_json_report "$JSON_PATH"
fi

if [[ $TOTAL_MISMATCHES -gt 0 ]]; then
    exit 1
elif [[ $TOTAL_VERIFIED -eq 0 ]]; then
    exit 3
else
    exit 0
fi
