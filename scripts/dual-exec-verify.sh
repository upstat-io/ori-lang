#!/bin/bash
# Dual-Execution Verification: Compare interpreter vs LLVM backend results
#
# Runs all spec tests through both backends and cross-references results
# to detect behavioral mismatches.
#
# Usage:
#   ./scripts/dual-exec-verify.sh [options] [test-path]
#
# Options:
#   -v, --verbose     Show all test results, not just mismatches
#   --json[=PATH]     Emit JSON report (default: build/dual-exec-report.json)
#   --main-only       Only run @main program comparison (skip @test comparison)
#   --test-only       Only run @test comparison (skip @main programs)
#   -h, --help        Show this help
#
# Exit codes:
#   0 = No behavioral mismatches detected
#   1 = Behavioral mismatches found (PASS in one backend, FAIL in other)
#   2 = Infrastructure error (build failure, binary not found)

set -uo pipefail
# Note: NOT using set -e because functions return mismatch counts as exit codes

# --- Configuration ---
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
INTERP_BIN="$ROOT_DIR/target/debug/ori"
LLVM_BIN="$ROOT_DIR/target/release/ori"
TEST_PATH="tests/"
VERBOSE=0
EMIT_JSON=0
JSON_PATH="$ROOT_DIR/build/dual-exec-report.json"
RUN_TESTS=1
RUN_MAIN=1

# --- Colors ---
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
DIM='\033[2m'
NC='\033[0m'

# --- Parse arguments ---
for arg in "$@"; do
    case $arg in
        -v|--verbose) VERBOSE=1 ;;
        --json) EMIT_JSON=1 ;;
        --json=*) EMIT_JSON=1; JSON_PATH="${arg#--json=}" ;;
        --main-only) RUN_TESTS=0 ;;
        --test-only) RUN_MAIN=0 ;;
        -h|--help)
            head -20 "$0" | tail -18 | sed 's/^# \?//'
            exit 0
            ;;
        *)
            if [[ -e "$arg" || -e "$ROOT_DIR/$arg" ]]; then
                TEST_PATH="$arg"
            else
                echo "Unknown argument: $arg" >&2
                exit 2
            fi
            ;;
    esac
done

# --- Verify binaries exist ---
check_binary() {
    local bin="$1" label="$2"
    if [[ ! -x "$bin" ]]; then
        echo -e "${RED}ERROR: $label binary not found at $bin${NC}" >&2
        echo "Run 'cargo build' (interpreter) or 'cargo blr' (LLVM) first." >&2
        exit 2
    fi
}

check_binary "$INTERP_BIN" "Interpreter"
check_binary "$LLVM_BIN" "LLVM"

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
                    echo -e "  ${GREEN}VERIFIED${NC}: $file :: $test"
                fi
                ;;
            PASS:BLOCKED|PASS:LCFAIL|PASS:MISSING)
                ((INTERP_ONLY++))
                ;;
            PASS:FAIL)
                ((MISMATCH_INTERP++))
                MISMATCH_DETAILS+=("${RED}MISMATCH${NC}: $file :: $test — PASS (interp) vs FAIL (LLVM)")
                ;;
            FAIL:PASS)
                ((MISMATCH_LLVM++))
                MISMATCH_DETAILS+=("${RED}MISMATCH${NC}: $file :: $test — FAIL (interp) vs PASS (LLVM)")
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
                    echo -e "  ${DIM}OTHER${NC}: $file :: $test — $status (interp) vs $llvm_status (LLVM)"
                fi
                ;;
        esac
    done < "$interp_parsed" || true
}

run_test_comparison() {
    echo -e "${BOLD}=== Dual-Execution Verification: @test functions ===${NC}"
    echo ""

    # Run interpreter
    echo -n "  Running interpreter backend..."
    ORI_LOG=off "$INTERP_BIN" test --verbose "$TEST_PATH" > "$INTERP_OUTPUT" 2>&1 || true
    local interp_summary
    interp_summary=$(grep -E "^  [0-9]+ passed" "$INTERP_OUTPUT" | tail -1)
    echo -e " ${GREEN}done${NC} ($interp_summary)"

    # Run LLVM
    echo -n "  Running LLVM backend..."
    ORI_LOG=off "$LLVM_BIN" test --verbose --backend=llvm "$TEST_PATH" > "$LLVM_OUTPUT" 2>&1 || true
    local llvm_summary
    llvm_summary=$(grep -E "^  [0-9]+ passed" "$LLVM_OUTPUT" | tail -1)
    echo -e " ${GREEN}done${NC} ($llvm_summary)"
    echo ""

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

    # compile_fail tests pass at type checker (no PASS: line in verbose output)
    # so they're verified by definition — both backends use the same type checker
    local compile_fail_verified=$(( llvm_total - VERIFIED ))
    if [[ $compile_fail_verified -lt 0 ]]; then compile_fail_verified=0; fi

    local total_verified=$((VERIFIED + compile_fail_verified))
    local total_interp=$((VERIFIED + INTERP_ONLY + MISMATCH_INTERP + BOTH_SKIP + BOTH_FAIL))
    local total_mismatches=$((MISMATCH_INTERP + MISMATCH_LLVM))

    echo -e "${BOLD}  Results:${NC}"
    echo -e "    ${GREEN}Verified (runtime, both PASS)${NC}:  $VERIFIED"
    echo -e "    ${GREEN}Verified (compile-fail)${NC}:        $compile_fail_verified"
    echo -e "    ${CYAN}LLVM coverage gap${NC}:              $INTERP_ONLY"
    echo -e "    ${DIM}Both skip${NC}:                      $BOTH_SKIP"
    if [[ $BOTH_FAIL -gt 0 ]]; then
        echo -e "    ${YELLOW}Both fail${NC}:                      $BOTH_FAIL"
    fi

    if [[ $total_mismatches -gt 0 ]]; then
        echo ""
        echo -e "    ${RED}${BOLD}BEHAVIORAL MISMATCHES: $total_mismatches${NC}"
        echo ""
        for detail in "${MISMATCH_DETAILS[@]}"; do
            echo -e "    $detail"
        done
    else
        echo ""
        echo -e "    ${GREEN}${BOLD}No behavioral mismatches detected${NC}"
    fi

    echo ""
    echo -e "  ${BOLD}Interpreter${NC}: $interp_total tests | ${BOLD}LLVM${NC}: $llvm_total pass + $llvm_lcfail compile-fail"
    echo -e "  ${BOLD}Total verified${NC}: $total_verified / $llvm_total ($((total_verified * 100 / (llvm_total > 0 ? llvm_total : 1)))%)"
    echo ""

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
    echo -e "${BOLD}=== Dual-Execution Verification: @main programs ===${NC}"
    echo ""

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
        echo -n "  $rel_file ... "

        # Run interpreter
        local interp_out interp_exit
        interp_out=$(ORI_LOG=off "$INTERP_BIN" run "$file" 2>&1) && interp_exit=0 || interp_exit=$?

        # Run AOT
        local aot_out aot_exit
        if ORI_LOG=off "$LLVM_BIN" build "$file" -o "$tmp_binary" 2>/dev/null; then
            aot_out=$("$tmp_binary" 2>&1) && aot_exit=0 || aot_exit=$?
            rm -f "$tmp_binary"
        else
            aot_exit=999  # compile failure
            aot_out="<AOT compile failed>"
        fi

        # Compare
        if [[ $aot_exit -eq 999 ]]; then
            echo -e "${YELLOW}AOT compile fail${NC}"
            ((MAIN_AOT_FAIL++))
        elif [[ $interp_exit -ne 0 ]] && [[ $aot_exit -ne 0 ]]; then
            echo -e "${DIM}both fail (interp=$interp_exit, aot=$aot_exit)${NC}"
            ((MAIN_INTERP_FAIL++))
        elif [[ "$interp_out" == "$aot_out" ]] && [[ $interp_exit -eq $aot_exit ]]; then
            echo -e "${GREEN}verified${NC}"
            ((MAIN_VERIFIED++))
        else
            echo -e "${RED}MISMATCH${NC}"
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
    echo -e "${BOLD}  Results:${NC}"
    echo -e "    ${GREEN}Verified${NC}:         $MAIN_VERIFIED"
    echo -e "    ${YELLOW}AOT compile fail${NC}: $MAIN_AOT_FAIL"
    if [[ $MAIN_INTERP_FAIL -gt 0 ]]; then
        echo -e "    ${DIM}Both fail${NC}:         $MAIN_INTERP_FAIL"
    fi

    if [[ $MAIN_MISMATCH -gt 0 ]]; then
        echo ""
        echo -e "    ${RED}${BOLD}BEHAVIORAL MISMATCHES: $MAIN_MISMATCH${NC}"
        for detail in "${MAIN_MISMATCH_DETAILS[@]}"; do
            echo -e "    $detail"
        done
    else
        echo ""
        echo -e "    ${GREEN}${BOLD}No behavioral mismatches detected${NC}"
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
echo -e "${BOLD}Ori Dual-Execution Verification${NC}"
echo -e "${DIM}Comparing interpreter vs LLVM backend for behavioral equivalence${NC}"
echo ""

TEST_MISMATCHES=0
MAIN_MISMATCHES=0

if [[ $RUN_TESTS -eq 1 ]]; then
    run_test_comparison || TEST_MISMATCHES=$?
fi

if [[ $RUN_MAIN -eq 1 ]]; then
    run_main_comparison || MAIN_MISMATCHES=$?
fi

# Final summary
TOTAL_MISMATCHES=$((TEST_MISMATCHES + MAIN_MISMATCHES))

echo "=============================================="
if [[ $TOTAL_MISMATCHES -eq 0 ]]; then
    echo -e "${GREEN}${BOLD}  DUAL-EXECUTION: ALL VERIFIED${NC}"
else
    echo -e "${RED}${BOLD}  DUAL-EXECUTION: $TOTAL_MISMATCHES MISMATCHES FOUND${NC}"
fi
echo "=============================================="
echo ""

if [[ $EMIT_JSON -eq 1 ]]; then
    emit_json_report "$JSON_PATH"
fi

exit $((TOTAL_MISMATCHES > 0 ? 1 : 0))
