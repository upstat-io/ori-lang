#!/bin/bash
# Identify which AIMS pipeline phase introduced an RC imbalance or structural change.
#
# Usage:
#   diagnostics/bisect-passes.sh [options] <file.ori>
#
# Options:
#   --function <name>  Filter to a specific function's phases
#   --rc-only          Suppress structural metrics columns (show only RC data)
#   --no-color         Disable color output
#   --color            Force color output (default: auto-detect terminal)
#   -h, --help         Show this help
#
# Output:
#   A per-function table showing how RC counts and structural metrics evolve
#   across AIMS pipeline phases:
#
#     Phase                          | RC Incs | RC Decs | Balance | Blocks | Vars | Delta
#     compute_var_reprs              |       0 |       0 |       0 |     10 |   19 |     0
#     realize_rc_reuse               |       0 |       4 |      -4 |     10 |   19 |    -4  ← first divergence
#     merge_blocks                   |       0 |       4 |      -4 |      7 |   22 |     0  (structural change)
#
#   Balance = rc_incs - rc_decs
#   Delta = change in balance from previous phase
#
#   The first phase where balance changes from 0 is highlighted as the potential
#   introduction point. Phases where blocks or vars count changes are also flagged.
#
# Data source:
#   Captures tracing events from the ori_arc::aims::pipeline target at info level.
#   Uses the structured checkpoint events emitted by trace_pipeline_checkpoint()
#   in the AIMS pipeline (compiler/ori_arc/src/pipeline/aims_pipeline/).
#   Does NOT regex-match LLVM IR for RC ops (SSOT: compiler checkpoint events).
#
# Environment:
#   ORI_BIN    Override path to ori binary (default: auto-detect via _common.sh)
#
# Exit codes:
#   0 = all phases clean (no RC divergence detected)
#   1 = divergence detected (RC balance changed from 0, or structural change)
#   2 = usage error or compilation failure

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/_common.sh"

# --- Defaults ---
FILTER_FUNCTION=""
RC_ONLY=0
USE_COLOR=auto
FILE=""

# --- Parse arguments ---
while [[ $# -gt 0 ]]; do
    case $1 in
        --function) FILTER_FUNCTION="$2"; shift 2 ;;
        --rc-only) RC_ONLY=1; shift ;;
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
    echo "Usage: diagnostics/bisect-passes.sh [options] <file.ori>" >&2
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
else
    C_RED="" C_GREEN="" C_YELLOW="" C_BOLD="" C_DIM="" C_NC=""
fi

# --- Locate ori binary ---
find_ori_bin
echo -e "${C_DIM}Using: $ORI${C_NC}" >&2

# --- Create temp directory with cleanup ---
TMPDIR_BP="$(mktemp -d)"
trap 'rm -rf "$TMPDIR_BP"' EXIT

# --- Compile with pipeline tracing enabled ---
echo -e "${C_DIM}Compiling with AIMS pipeline tracing...${C_NC}" >&2
TRACE_OUTPUT="$TMPDIR_BP/trace.log"
BUILD_OUTPUT="$TMPDIR_BP/bisect-bin"

if ! ORI_LOG=ori_arc::aims::pipeline=info "$ORI" build "$FILE" -o "$BUILD_OUTPUT" 2>"$TRACE_OUTPUT"; then
    echo "Error: compilation failed" >&2
    cat "$TRACE_OUTPUT" >&2
    exit 2
fi

# --- Strip ANSI codes from trace output ---
CLEAN_TRACE="$TMPDIR_BP/trace-clean.log"
sed 's/\x1b\[[0-9;]*m//g' "$TRACE_OUTPUT" > "$CLEAN_TRACE"

# --- Extract checkpoint events ---
EVENTS="$TMPDIR_BP/events.tsv"
grep "AIMS phase checkpoint" "$CLEAN_TRACE" | \
    sed -n 's/.*function="\([^"]*\)".*phase="\([^"]*\)".*rc_incs=\([0-9]*\).*rc_decs=\([0-9]*\).*blocks=\([0-9]*\).*vars=\([0-9]*\).*/\1\t\2\t\3\t\4\t\5\t\6/p' \
    > "$EVENTS"

if [[ ! -s "$EVENTS" ]]; then
    echo "Error: no AIMS pipeline checkpoint events found in trace output" >&2
    echo "This may indicate the program has no functions that go through the AIMS pipeline." >&2
    exit 2
fi

# --- Get list of functions ---
FUNCTIONS="$TMPDIR_BP/functions.txt"
cut -f1 "$EVENTS" | sort -u > "$FUNCTIONS"

if [[ -n "$FILTER_FUNCTION" ]]; then
    if ! grep -q "^${FILTER_FUNCTION}$" "$FUNCTIONS"; then
        echo "Error: function '$FILTER_FUNCTION' not found in checkpoint events" >&2
        echo "Available functions:" >&2
        sed 's/^/  /' "$FUNCTIONS" >&2
        exit 2
    fi
fi

# --- Run the built binary and check for issues ---
RUN_STATUS=0
LEAK_OUTPUT="$TMPDIR_BP/leak.log"
if ORI_CHECK_LEAKS=1 "$BUILD_OUTPUT" > /dev/null 2>"$LEAK_OUTPUT"; then
    : # clean run
else
    RUN_STATUS=$?
fi

# --- Display per-function tables ---
HAS_DIVERGENCE=0

while IFS= read -r func_name; do
    # Apply function filter
    if [[ -n "$FILTER_FUNCTION" ]] && [[ "$func_name" != "$FILTER_FUNCTION" ]]; then
        continue
    fi

    echo ""
    echo -e "${C_BOLD}Function: $func_name${C_NC}"

    # Print header
    if [[ "$RC_ONLY" -eq 1 ]]; then
        printf "  %-35s | %7s | %7s | %7s | %5s\n" "Phase" "RC Incs" "RC Decs" "Balance" "Delta"
        printf "  %-35s-+-%7s-+-%7s-+-%7s-+-%5s\n" "-----------------------------------" "-------" "-------" "-------" "-----"
    else
        printf "  %-35s | %7s | %7s | %7s | %6s | %4s | %5s\n" "Phase" "RC Incs" "RC Decs" "Balance" "Blocks" "Vars" "Delta"
        printf "  %-35s-+-%7s-+-%7s-+-%7s-+-%6s-+-%4s-+-%5s\n" "-----------------------------------" "-------" "-------" "-------" "------" "----" "-----"
    fi

    PREV_BALANCE=0
    PREV_BLOCKS=""
    PREV_VARS=""
    FIRST_DIVERGENCE=""

    # Process events for this function (process substitution avoids subshell)
    while IFS=$'\t' read -r _fn phase rc_incs rc_decs blocks vars; do
        balance=$((rc_incs - rc_decs))
        delta=$((balance - PREV_BALANCE))

        # Detect first RC divergence
        MARKER=""
        if [[ "$delta" -ne 0 ]] && [[ -z "$FIRST_DIVERGENCE" ]]; then
            FIRST_DIVERGENCE="$phase"
            MARKER=" ${C_RED}← RC divergence${C_NC}"
            HAS_DIVERGENCE=1
        elif [[ "$delta" -ne 0 ]]; then
            MARKER=" ${C_YELLOW}← RC change${C_NC}"
        fi

        # Detect structural changes
        if [[ -n "$PREV_BLOCKS" ]] && [[ "$blocks" != "$PREV_BLOCKS" || "$vars" != "$PREV_VARS" ]]; then
            if [[ -z "$MARKER" ]]; then
                MARKER=" ${C_YELLOW}← structural${C_NC}"
            else
                MARKER="$MARKER ${C_YELLOW}+structural${C_NC}"
            fi
            HAS_DIVERGENCE=1
        fi

        # Color the balance
        if [[ "$balance" -eq 0 ]]; then
            BAL_COLOR="$C_GREEN"
        else
            BAL_COLOR="$C_RED"
        fi

        if [[ "$RC_ONLY" -eq 1 ]]; then
            printf "  %-35s | %7d | %7d | ${BAL_COLOR}%7d${C_NC} | %5d${MARKER}\n" \
                "$phase" "$rc_incs" "$rc_decs" "$balance" "$delta"
        else
            printf "  %-35s | %7d | %7d | ${BAL_COLOR}%7d${C_NC} | %6d | %4d | %5d${MARKER}\n" \
                "$phase" "$rc_incs" "$rc_decs" "$balance" "$blocks" "$vars" "$delta"
        fi

        PREV_BALANCE=$balance
        PREV_BLOCKS=$blocks
        PREV_VARS=$vars
    done < <(grep "^${func_name}	" "$EVENTS")

done < "$FUNCTIONS"

# --- Runtime status ---
echo ""
if [[ "$RUN_STATUS" -ne 0 ]]; then
    echo -e "${C_RED}${C_BOLD}Runtime: program exited with code $RUN_STATUS${C_NC}"
    HAS_DIVERGENCE=1
fi

if [[ -s "$LEAK_OUTPUT" ]] && grep -q "LEAK" "$LEAK_OUTPUT"; then
    echo -e "${C_RED}${C_BOLD}Leak check: leaks detected${C_NC}"
    sed 's/^/  /' "$LEAK_OUTPUT"
    HAS_DIVERGENCE=1
else
    echo -e "${C_GREEN}Leak check: clean${C_NC}"
fi

# --- Exit code ---
if [[ "$HAS_DIVERGENCE" -ne 0 ]]; then
    exit 1
fi
exit 0
