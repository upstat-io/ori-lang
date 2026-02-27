#!/bin/bash
# All-in-one AOT diagnostic for an Ori source file.
#
# Usage:
#   diagnostics/diagnose-aot.sh [options] <file.ori>
#
# Options:
#   --valgrind         Run under Valgrind for memory error detection
#   --rc-trace         Enable ORI_TRACE_RC=1 during execution and leak check
#   --verbose          Include disassembly in the report
#   --no-color         Disable color output
#   --color            Force color output (default: auto-detect terminal)
#   -h, --help         Show this help
#
# Runs a battery of diagnostics on an Ori program:
#   1. Compilation   — Build with ori build, capture timing
#   2. Execution     — Run the binary, capture exit code and output
#   3. Leak Check    — Run with ORI_CHECK_LEAKS=1
#   4. RC Stats      — Analyze IR for RC operation balance
#   5. LLVM IR       — Save IR for manual inspection
#   6. Valgrind      — (if --valgrind) Memory error detection
#   7. Disassembly   — (if --verbose) Native disassembly saved
#
# Environment:
#   ORI_BIN    Override path to ori binary
#
# Requires: ir-dump.sh, rc-stats.sh, disasm-ori.sh (in same directory)
#
# Exit codes:
#   0 = all checks pass
#   1 = one or more checks failed
#   2 = usage error

set -uo pipefail
# Note: no -e because we intentionally handle failures per-section

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/_common.sh"
find_ori_bin

# --- Defaults ---
USE_VALGRIND=0
USE_RC_TRACE=0
VERBOSE=0
USE_COLOR=auto
FILE=""

# --- Parse arguments ---
while [[ $# -gt 0 ]]; do
    case $1 in
        --valgrind) USE_VALGRIND=1; shift ;;
        --rc-trace) USE_RC_TRACE=1; shift ;;
        --verbose) VERBOSE=1; shift ;;
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
            if [[ -n "$FILE" ]]; then
                echo "Error: multiple files specified" >&2
                exit 2
            fi
            FILE="$1"; shift
            ;;
    esac
done

if [[ -z "$FILE" ]]; then
    echo "Error: no input file specified" >&2
    echo "Usage: diagnostics/diagnose-aot.sh [options] <file.ori>" >&2
    exit 2
fi

if [[ ! -f "$FILE" ]]; then
    echo "Error: file not found: $FILE" >&2
    exit 2
fi

# --- Resolve color mode ---
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
    C_BOLD='\033[1m'
    C_DIM='\033[2m'
    C_NC='\033[0m'
    SYM_PASS="${C_GREEN}PASS${C_NC}"
    SYM_FAIL="${C_RED}FAIL${C_NC}"
    SYM_WARN="${C_YELLOW}WARN${C_NC}"
    SYM_SKIP="${C_DIM}SKIP${C_NC}"
    SYM_INFO="${C_DIM}INFO${C_NC}"
else
    C_RED="" C_GREEN="" C_YELLOW="" C_BOLD="" C_DIM="" C_NC=""
    SYM_PASS="PASS"
    SYM_FAIL="FAIL"
    SYM_WARN="WARN"
    SYM_SKIP="SKIP"
    SYM_INFO="INFO"
fi

# --- Setup ---
tmpdir=$(mktemp -d)
trap 'rm -rf "$tmpdir"' EXIT
basename_file="$(basename "$FILE")"
has_failure=0

# Results array (indexed by section number)
declare -a results
declare -a result_details

echo ""
echo -e "${C_BOLD}Diagnostic Report: ${basename_file}${C_NC}"
echo "════════════════════════════════════════════════════════"
echo ""

# ═══════════════════════════════════════════════════════════
# Section 1: Compilation
# ═══════════════════════════════════════════════════════════
echo -e "${C_BOLD}[1/7] Compilation${C_NC}"

start_time=$(date +%s%N 2>/dev/null || python3 -c "import time; print(int(time.time()*1e9))")
if "$ORI" build "$FILE" -o "$tmpdir/binary" 2>"$tmpdir/build_err.txt"; then
    end_time=$(date +%s%N 2>/dev/null || python3 -c "import time; print(int(time.time()*1e9))")
    elapsed_ms=$(( (end_time - start_time) / 1000000 ))
    elapsed_s=$(awk "BEGIN { printf \"%.2f\", $elapsed_ms / 1000 }")
    results[1]="$SYM_PASS"
    result_details[1]="${elapsed_s}s"
    echo -e "  ${SYM_PASS}  Built in ${elapsed_s}s"
else
    results[1]="$SYM_FAIL"
    result_details[1]="build failed"
    has_failure=1
    echo -e "  ${SYM_FAIL}  Build failed:"
    sed 's/^/  │ /' "$tmpdir/build_err.txt"
    echo ""
    echo -e "${C_BOLD}Summary${C_NC}"
    echo "────────────────────────────────────────────────────────"
    echo -e "  [${results[1]}]  Compilation  ${result_details[1]}"
    echo -e "  [${SYM_SKIP}]  Remaining sections skipped (build failed)"
    echo ""
    exit 1
fi
echo ""

# ═══════════════════════════════════════════════════════════
# Section 2: Execution
# ═══════════════════════════════════════════════════════════
echo -e "${C_BOLD}[2/7] Execution${C_NC}"

rc_trace_env=()
if [[ $USE_RC_TRACE -eq 1 ]]; then
    rc_trace_env=(env ORI_TRACE_RC=1)
    echo -e "  ${C_DIM}(ORI_TRACE_RC=1 enabled)${C_NC}"
fi
"${rc_trace_env[@]}" "$tmpdir/binary" > "$tmpdir/exec_stdout.txt" 2>"$tmpdir/exec_stderr.txt"
exec_exit=$?

if [[ $exec_exit -eq 0 ]]; then
    results[2]="$SYM_PASS"
    result_details[2]="exit=0"
    echo -e "  ${SYM_PASS}  Exit code: 0"
else
    results[2]="$SYM_FAIL"
    result_details[2]="exit=${exec_exit}"
    has_failure=1
    echo -e "  ${SYM_FAIL}  Exit code: ${exec_exit}"
fi

stdout_size=$(wc -c < "$tmpdir/exec_stdout.txt")
stderr_size=$(wc -c < "$tmpdir/exec_stderr.txt")
if [[ $stdout_size -gt 0 ]]; then
    echo -e "  ${C_DIM}stdout (${stdout_size} bytes):${C_NC}"
    head -10 "$tmpdir/exec_stdout.txt" | sed 's/^/  │ /'
    [[ $(wc -l < "$tmpdir/exec_stdout.txt") -gt 10 ]] && echo "  │ ... (truncated)"
fi
if [[ $stderr_size -gt 0 ]]; then
    echo -e "  ${C_DIM}stderr (${stderr_size} bytes):${C_NC}"
    head -10 "$tmpdir/exec_stderr.txt" | sed 's/^/  │ /'
    [[ $(wc -l < "$tmpdir/exec_stderr.txt") -gt 10 ]] && echo "  │ ... (truncated)"
fi
echo ""

# ═══════════════════════════════════════════════════════════
# Section 3: Leak Check
# ═══════════════════════════════════════════════════════════
echo -e "${C_BOLD}[3/7] Leak Check${C_NC}"

leak_env=(env ORI_CHECK_LEAKS=1)
if [[ $USE_RC_TRACE -eq 1 ]]; then
    leak_env+=(ORI_TRACE_RC=1)
fi
"${leak_env[@]}" "$tmpdir/binary" > /dev/null 2>"$tmpdir/leak_stderr.txt"
leak_exit=$?

if grep -q "RC live count" "$tmpdir/leak_stderr.txt"; then
    # Extract the live count from the output
    live_count=$(grep "RC live count" "$tmpdir/leak_stderr.txt" | grep -oP '\d+' | tail -1)
    if [[ "$live_count" == "0" ]]; then
        results[3]="$SYM_PASS"
        result_details[3]="0 leaks"
        echo -e "  ${SYM_PASS}  No RC leaks (live count: 0)"
    else
        results[3]="$SYM_FAIL"
        result_details[3]="${live_count} leaked"
        has_failure=1
        echo -e "  ${SYM_FAIL}  ${live_count} RC allocations not freed"
        grep "RC" "$tmpdir/leak_stderr.txt" | sed 's/^/  │ /'
    fi
else
    results[3]="$SYM_PASS"
    result_details[3]="clean (no RC output)"
    echo -e "  ${SYM_PASS}  Clean exit (no RC tracking output)"
fi
echo ""

# ═══════════════════════════════════════════════════════════
# Section 4: RC Stats
# ═══════════════════════════════════════════════════════════
echo -e "${C_BOLD}[4/7] RC Stats${C_NC}"

color_flag="--no-color"
[[ "$USE_COLOR" == "yes" ]] && color_flag="--color"

if "$SCRIPT_DIR/rc-stats.sh" "$color_flag" "$FILE" > "$tmpdir/rc_stats.txt" 2>/dev/null; then
    results[4]="$SYM_PASS"
    result_details[4]="balanced"
    echo -e "  ${SYM_PASS}  All functions balanced"
else
    results[4]="$SYM_WARN"
    result_details[4]="imbalanced"
    echo -e "  ${SYM_WARN}  Imbalanced functions detected:"
fi
sed 's/^/  │ /' "$tmpdir/rc_stats.txt"
echo ""

# ═══════════════════════════════════════════════════════════
# Section 5: LLVM IR
# ═══════════════════════════════════════════════════════════
echo -e "${C_BOLD}[5/7] LLVM IR${C_NC}"

ir_file="${tmpdir}/ir-${basename_file%.ori}.ll"
if "$SCRIPT_DIR/ir-dump.sh" --raw "$FILE" > "$ir_file" 2>/dev/null; then
    ir_lines=$(wc -l < "$ir_file")
    results[5]="$SYM_INFO"
    result_details[5]="${ir_lines} lines → ${ir_file}"
    echo -e "  ${SYM_INFO}  ${ir_lines} lines saved to ${ir_file}"
else
    results[5]="$SYM_FAIL"
    result_details[5]="IR capture failed"
    echo -e "  ${SYM_FAIL}  Failed to capture LLVM IR"
fi
echo ""

# ═══════════════════════════════════════════════════════════
# Section 6: Valgrind
# ═══════════════════════════════════════════════════════════
echo -e "${C_BOLD}[6/7] Valgrind${C_NC}"

if [[ "$USE_VALGRIND" -eq 1 ]]; then
    if command -v valgrind &>/dev/null; then
        valgrind --leak-check=full --error-exitcode=42 \
            "$tmpdir/binary" > /dev/null 2>"$tmpdir/valgrind.txt"
        vg_exit=$?
        if [[ $vg_exit -eq 42 ]]; then
            results[6]="$SYM_FAIL"
            result_details[6]="errors detected"
            has_failure=1
            echo -e "  ${SYM_FAIL}  Valgrind found errors:"
            grep -E "^==[0-9]+==" "$tmpdir/valgrind.txt" | tail -20 | sed 's/^/  │ /'
        else
            results[6]="$SYM_PASS"
            result_details[6]="clean"
            echo -e "  ${SYM_PASS}  No memory errors detected"
        fi
    else
        results[6]="$SYM_SKIP"
        result_details[6]="valgrind not installed"
        echo -e "  ${SYM_SKIP}  Valgrind not found (install valgrind)"
    fi
else
    results[6]="$SYM_SKIP"
    result_details[6]="use --valgrind"
    echo -e "  ${SYM_SKIP}  Skipped (use --valgrind to enable)"
fi
echo ""

# ═══════════════════════════════════════════════════════════
# Section 7: Disassembly
# ═══════════════════════════════════════════════════════════
echo -e "${C_BOLD}[7/7] Disassembly${C_NC}"

if [[ "$VERBOSE" -eq 1 ]]; then
    disasm_file="${tmpdir}/disasm-${basename_file%.ori}.txt"
    if "$SCRIPT_DIR/disasm-ori.sh" --no-color "$FILE" > "$disasm_file" 2>/dev/null; then
        disasm_lines=$(wc -l < "$disasm_file")
        results[7]="$SYM_INFO"
        result_details[7]="${disasm_lines} lines → ${disasm_file}"
        echo -e "  ${SYM_INFO}  ${disasm_lines} lines saved to ${disasm_file}"
    else
        results[7]="$SYM_FAIL"
        result_details[7]="disassembly failed"
        echo -e "  ${SYM_FAIL}  Failed to disassemble"
    fi
else
    results[7]="$SYM_SKIP"
    result_details[7]="use --verbose"
    echo -e "  ${SYM_SKIP}  Skipped (use --verbose to enable)"
fi
echo ""

# ═══════════════════════════════════════════════════════════
# Summary
# ═══════════════════════════════════════════════════════════
echo -e "${C_BOLD}Summary${C_NC}"
echo "════════════════════════════════════════════════════════"

section_names=("" "Compilation" "Execution" "Leak Check" "RC Stats" "LLVM IR" "Valgrind" "Disassembly")
for i in 1 2 3 4 5 6 7; do
    printf "  [%b]  %-14s %s\n" "${results[$i]}" "${section_names[$i]}" "${result_details[$i]}"
done

echo ""
if [[ $has_failure -eq 0 ]]; then
    echo -e "${C_GREEN}All checks passed.${C_NC}"
else
    echo -e "${C_RED}One or more checks failed.${C_NC}"
fi

exit $has_failure
