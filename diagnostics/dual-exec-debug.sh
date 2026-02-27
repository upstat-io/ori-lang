#!/bin/bash
# Backend comparison diagnostic for a single Ori source file.
#
# Usage:
#   diagnostics/dual-exec-debug.sh [options] <file.ori>
#
# Options:
#   --verbose          Add ORI_LOG=debug to both runs for trace comparison
#   --no-color         Disable color output
#   --color            Force color output (default: auto-detect terminal)
#   -h, --help         Show this help
#
# Runs a program through both the interpreter (ori run) and AOT backend
# (ori build + execute), comparing exit codes and stdout. When results
# differ, automatically runs ir-dump.sh and rc-stats.sh for diagnostics.
#
# Environment:
#   ORI_BIN    Override path to ori binary
#
# Requires: ir-dump.sh, rc-stats.sh (in same directory)
#
# Exit codes:
#   0 = both backends match
#   1 = mismatch detected
#   2 = usage error or infrastructure failure

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/_common.sh"
find_ori_bin

# --- Defaults ---
VERBOSE=0
USE_COLOR=auto
FILE=""

# --- Parse arguments ---
while [[ $# -gt 0 ]]; do
    case $1 in
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
    echo "Usage: diagnostics/dual-exec-debug.sh [options] <file.ori>" >&2
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

# --- Setup ---
tmpdir=$(mktemp -d)
trap 'rm -rf "$tmpdir"' EXIT
basename_file="$(basename "$FILE")"

# --- ORI_LOG for verbose mode ---
LOG_ENV=""
if [[ "$VERBOSE" -eq 1 ]]; then
    LOG_ENV="ORI_LOG=debug"
fi

echo ""
echo -e "${C_BOLD}Dual Execution: ${basename_file}${C_NC}"
echo "════════════════════════════════════════════════════════"
echo ""

# ═══════════════════════════════════════════════════════════
# Step 1: Run interpreter
# ═══════════════════════════════════════════════════════════
echo -e "${C_BOLD}[1/2] Interpreter (ori run)${C_NC}"

interp_start=$(date +%s%N 2>/dev/null || python3 -c "import time; print(int(time.time()*1e9))")
if [[ -n "$LOG_ENV" ]]; then
    env $LOG_ENV "$ORI" run "$FILE" \
        > "$tmpdir/interp_stdout.txt" \
        2> "$tmpdir/interp_stderr.txt"
    interp_exit=$?
else
    "$ORI" run "$FILE" \
        > "$tmpdir/interp_stdout.txt" \
        2> "$tmpdir/interp_stderr.txt"
    interp_exit=$?
fi
interp_end=$(date +%s%N 2>/dev/null || python3 -c "import time; print(int(time.time()*1e9))")
interp_ms=$(( (interp_end - interp_start) / 1000000 ))
interp_s=$(awk "BEGIN { printf \"%.2f\", $interp_ms / 1000 }")

interp_stdout=$(cat "$tmpdir/interp_stdout.txt")
interp_stdout_display="${interp_stdout:-"(empty)"}"
# Escape for display (show \n literally for newlines)
interp_stdout_oneline=$(echo "$interp_stdout" | head -5 | tr '\n' '\\' | sed 's/\\/\\n/g; s/\\n$//')
[[ -z "$interp_stdout_oneline" ]] && interp_stdout_oneline="(empty)"

echo -e "  exit=${interp_exit}  stdout=${C_DIM}\"${interp_stdout_oneline}\"${C_NC}  (${interp_s}s)"

if [[ "$VERBOSE" -eq 1 ]] && [[ -s "$tmpdir/interp_stderr.txt" ]]; then
    echo -e "  ${C_DIM}stderr (trace):${C_NC}"
    head -20 "$tmpdir/interp_stderr.txt" | sed 's/^/  │ /'
    stderr_lines=$(wc -l < "$tmpdir/interp_stderr.txt")
    if [[ "$stderr_lines" -gt 20 ]]; then
        echo -e "  │ ${C_DIM}... ($stderr_lines total lines, truncated)${C_NC}"
    fi
fi
echo ""

# ═══════════════════════════════════════════════════════════
# Step 2: Build and run AOT
# ═══════════════════════════════════════════════════════════
echo -e "${C_BOLD}[2/2] AOT (ori build + execute)${C_NC}"

aot_binary="$tmpdir/aot_binary"

# Build step
if ! "$ORI" build "$FILE" -o "$aot_binary" 2>"$tmpdir/build_err.txt"; then
    echo -e "  ${C_RED}Build failed${C_NC}:"
    sed 's/^/  │ /' "$tmpdir/build_err.txt"
    echo ""
    echo -e "${C_BOLD}Summary${C_NC}"
    echo "════════════════════════════════════════════════════════"
    echo -e "  Interpreter:  exit=${interp_exit}  (${interp_s}s)"
    echo -e "  AOT:          ${C_RED}BUILD FAILED${C_NC}"
    echo ""
    echo -e "${C_RED}Cannot compare: AOT compilation failed.${C_NC}"
    exit 1
fi

# Execute step
aot_start=$(date +%s%N 2>/dev/null || python3 -c "import time; print(int(time.time()*1e9))")
if [[ -n "$LOG_ENV" ]]; then
    env $LOG_ENV "$aot_binary" \
        > "$tmpdir/aot_stdout.txt" \
        2> "$tmpdir/aot_stderr.txt"
    aot_exit=$?
else
    "$aot_binary" \
        > "$tmpdir/aot_stdout.txt" \
        2> "$tmpdir/aot_stderr.txt"
    aot_exit=$?
fi
aot_end=$(date +%s%N 2>/dev/null || python3 -c "import time; print(int(time.time()*1e9))")
aot_ms=$(( (aot_end - aot_start) / 1000000 ))
aot_s=$(awk "BEGIN { printf \"%.2f\", $aot_ms / 1000 }")

aot_stdout=$(cat "$tmpdir/aot_stdout.txt")
aot_stdout_oneline=$(echo "$aot_stdout" | head -5 | tr '\n' '\\' | sed 's/\\/\\n/g; s/\\n$//')
[[ -z "$aot_stdout_oneline" ]] && aot_stdout_oneline="(empty)"

echo -e "  exit=${aot_exit}  stdout=${C_DIM}\"${aot_stdout_oneline}\"${C_NC}  (${aot_s}s)"

if [[ "$VERBOSE" -eq 1 ]] && [[ -s "$tmpdir/aot_stderr.txt" ]]; then
    echo -e "  ${C_DIM}stderr (trace):${C_NC}"
    head -20 "$tmpdir/aot_stderr.txt" | sed 's/^/  │ /'
    stderr_lines=$(wc -l < "$tmpdir/aot_stderr.txt")
    if [[ "$stderr_lines" -gt 20 ]]; then
        echo -e "  │ ${C_DIM}... ($stderr_lines total lines, truncated)${C_NC}"
    fi
fi
echo ""

# ═══════════════════════════════════════════════════════════
# Comparison
# ═══════════════════════════════════════════════════════════
echo -e "${C_BOLD}Comparison${C_NC}"
echo "────────────────────────────────────────────────────────"

has_mismatch=0

# Compare exit codes
if [[ $interp_exit -eq $aot_exit ]]; then
    echo -e "  Exit codes:  ${C_GREEN}MATCH${C_NC} (both ${interp_exit})"
else
    echo -e "  Exit codes:  ${C_RED}MISMATCH${C_NC} (interpreter=${interp_exit}, AOT=${aot_exit})"
    has_mismatch=1
fi

# Compare stdout
if [[ "$interp_stdout" == "$aot_stdout" ]]; then
    echo -e "  Stdout:      ${C_GREEN}MATCH${C_NC}"
else
    echo -e "  Stdout:      ${C_RED}MISMATCH${C_NC}"
    has_mismatch=1
    # Show diff
    echo ""
    echo -e "  ${C_BOLD}Stdout diff:${C_NC}"
    diff --color=never \
        <(echo "$interp_stdout") \
        <(echo "$aot_stdout") \
        | sed 's/^/  │ /' || true
fi

echo ""

# ═══════════════════════════════════════════════════════════
# Auto-diagnostics on mismatch
# ═══════════════════════════════════════════════════════════
if [[ $has_mismatch -eq 1 ]]; then
    echo -e "${C_BOLD}Auto-diagnostics${C_NC}"
    echo "────────────────────────────────────────────────────────"

    # Run ir-dump.sh
    ir_file="$tmpdir/diag-ir.ll"
    if "$SCRIPT_DIR/ir-dump.sh" --raw "$FILE" > "$ir_file" 2>/dev/null; then
        ir_lines=$(wc -l < "$ir_file")
        echo -e "  LLVM IR saved to ${ir_file} (${ir_lines} lines)"
    else
        echo -e "  ${C_YELLOW}IR dump failed${C_NC}"
    fi

    # Run rc-stats.sh
    color_flag="--no-color"
    [[ "$USE_COLOR" == "yes" ]] && color_flag="--color"
    rc_output=$("$SCRIPT_DIR/rc-stats.sh" "$color_flag" "$FILE" 2>/dev/null) || true
    if [[ -n "$rc_output" ]]; then
        echo -e "  RC Stats:"
        echo "$rc_output" | sed 's/^/  │ /'
    else
        echo -e "  ${C_YELLOW}RC stats unavailable${C_NC}"
    fi

    echo ""
fi

# ═══════════════════════════════════════════════════════════
# Summary
# ═══════════════════════════════════════════════════════════
echo -e "${C_BOLD}Summary${C_NC}"
echo "════════════════════════════════════════════════════════"
echo -e "  Interpreter:  exit=${interp_exit}  stdout=${C_DIM}\"${interp_stdout_oneline}\"${C_NC}  (${interp_s}s)"
echo -e "  AOT:          exit=${aot_exit}  stdout=${C_DIM}\"${aot_stdout_oneline}\"${C_NC}  (${aot_s}s)"
echo ""
if [[ $has_mismatch -eq 0 ]]; then
    echo -e "${C_GREEN}MATCH: Both backends produce identical results.${C_NC}"
else
    echo -e "${C_RED}MISMATCH: Backends produce different results.${C_NC}"
fi

exit $has_mismatch
