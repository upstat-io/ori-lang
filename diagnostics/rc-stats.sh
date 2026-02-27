#!/bin/bash
# Count RC and COW operations in LLVM IR per function.
#
# Usage:
#   diagnostics/rc-stats.sh [options] <file.ori>
#
# Options:
#   --optimized        Analyze optimized IR (passed through to ir-dump.sh)
#   --no-color         Disable color output
#   --color            Force color output (default: auto-detect terminal)
#   -h, --help         Show this help
#
# Output:
#   A table summarizing RC operations (alloc, inc, dec, free) and COW
#   operations per function, with a balance column. Imbalanced functions
#   are flagged with a warning.
#
#   Balance = (alloc + inc) - (dec + free)
#     Positive → potential leak (more retains than releases)
#     Negative → potential over-release (more releases than retains)
#     Zero → balanced (not guaranteed correct, but a good sign)
#
# Environment:
#   ORI_BIN    Override path to ori binary (passed through to ir-dump.sh)
#
# Requires: ir-dump.sh (in same directory), ori compiler with LLVM support
#
# Exit codes:
#   0 = success (all functions balanced)
#   1 = imbalanced functions detected
#   2 = usage error or compilation failure

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# --- Defaults ---
OPTIMIZED=0
USE_COLOR=auto
FILE=""

# --- Parse arguments ---
while [[ $# -gt 0 ]]; do
    case $1 in
        --optimized) OPTIMIZED=1; shift ;;
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
    echo "Usage: diagnostics/rc-stats.sh [options] <file.ori>" >&2
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

# --- Capture IR ---
dump_args=(--raw)
if [[ "$OPTIMIZED" -eq 1 ]]; then
    dump_args+=(--optimized)
fi

tmpdir=$(mktemp -d)
trap 'rm -rf "$tmpdir"' EXIT

echo "Analyzing $(basename "$FILE")..." >&2
if ! "$SCRIPT_DIR/ir-dump.sh" "${dump_args[@]}" "$FILE" > "$tmpdir/ir.ll" 2>"$tmpdir/err.txt"; then
    cat "$tmpdir/err.txt" >&2
    echo "Error: compilation failed" >&2
    exit 2
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

# --- Parse IR and count operations per function ---
awk \
    -v red="$C_RED" -v green="$C_GREEN" -v yellow="$C_YELLOW" \
    -v bold="$C_BOLD" -v dim="$C_DIM" -v nc="$C_NC" \
'
BEGIN {
    fn_count = 0
    total_alloc = 0; total_inc = 0; total_dec = 0; total_free = 0; total_cow = 0
}

# Track current function
/^define / {
    fn_count++
    # Extract function name: define ... @name( or @"name"(
    match($0, /@"?([^("]+)"?\(/, m)
    current_fn = m[1]
    # Demangle Ori names
    demangled = current_fn
    if (demangled ~ /^_ori_drop\$/) {
        sub(/^_ori_drop\$/, "drop<", demangled)
        demangled = demangled ">"
    } else if (demangled ~ /^_ori_/) {
        sub(/^_ori_/, "@", demangled)
        gsub(/\$\$/, " impl ", demangled)
        gsub(/\$/, ".", demangled)
    }
    fn_names[fn_count] = demangled
    fn_alloc[fn_count] = 0
    fn_inc[fn_count] = 0
    fn_dec[fn_count] = 0
    fn_free[fn_count] = 0
    fn_cow[fn_count] = 0
}

# Count RC operations within current function
/call.*ori_rc_alloc/ { fn_alloc[fn_count]++ }
/call.*ori_rc_inc/   { fn_inc[fn_count]++ }
/call.*ori_rc_dec/   { fn_dec[fn_count]++ }
/call.*ori_rc_free/  { fn_free[fn_count]++ }

# Count COW operations
/call.*ori_list_[a-z_]*_cow/ { fn_cow[fn_count]++ }
/call.*ori_str_[a-z_]*_cow/  { fn_cow[fn_count]++ }
/call.*ori_map_[a-z_]*_cow/  { fn_cow[fn_count]++ }
/call.*ori_set_[a-z_]*_cow/  { fn_cow[fn_count]++ }

END {
    # Print header
    printf "%s%-30s  alloc  inc   dec   free  cow   balance%s\n", bold, "Function", nc
    printf "%-30s  -----  ----  ----  ----  ----  -------\n", "──────────────────────────────"

    has_imbalance = 0

    for (i = 1; i <= fn_count; i++) {
        a = fn_alloc[i]; inc = fn_inc[i]; d = fn_dec[i]; f = fn_free[i]; c = fn_cow[i]
        # Skip functions with no RC operations
        if (a + inc + d + f + c == 0) continue

        balance = (a + inc) - (d + f)
        total_alloc += a; total_inc += inc; total_dec += d; total_free += f; total_cow += c

        # Format balance with color
        if (balance > 0) {
            bal_str = sprintf("%s+%d%s", yellow, balance, nc)
            flag = sprintf(" %s⚠ leak?%s", yellow, nc)
            has_imbalance = 1
        } else if (balance < 0) {
            bal_str = sprintf("%s%d%s", red, balance, nc)
            flag = sprintf(" %s⚠ over-release?%s", red, nc)
            has_imbalance = 1
        } else {
            bal_str = sprintf("%s0%s", green, nc)
            flag = ""
        }

        printf "%-30s  %5d  %4d  %4d  %4d  %4d  %s%s\n", fn_names[i], a, inc, d, f, c, bal_str, flag
    }

    # Totals
    total_bal = (total_alloc + total_inc) - (total_dec + total_free)
    printf "%-30s  -----  ----  ----  ----  ----  -------\n", "──────────────────────────────"
    if (total_bal > 0) {
        bal_str = sprintf("%s+%d%s", yellow, total_bal, nc)
    } else if (total_bal < 0) {
        bal_str = sprintf("%s%d%s", red, total_bal, nc)
    } else {
        bal_str = sprintf("%s0%s", green, nc)
    }
    printf "%s%-30s  %5d  %4d  %4d  %4d  %4d  %s%s\n", bold, "TOTAL", total_alloc, total_inc, total_dec, total_free, total_cow, bal_str, nc

    # Exit code: 1 if any imbalance detected
    exit has_imbalance
}
' "$tmpdir/ir.ll"
