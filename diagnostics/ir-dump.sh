#!/bin/bash
# Dump annotated LLVM IR for an Ori source file.
#
# Usage:
#   diagnostics/ir-dump.sh [options] <file.ori>
#
# Options:
#   --raw              Output raw IR without annotations or color
#   --color            Force color output (default: auto-detect terminal)
#   --no-color         Disable color output
#   --optimized        Show optimized IR (via --emit=llvm-ir) instead of raw codegen output
#   --function <name>  Show only the named function
#   -h, --help         Show this help
#
# The default mode captures unoptimized IR (immediately after codegen, before
# LLVM optimization passes) via ORI_DEBUG_LLVM=1. Use --optimized to see the
# IR after LLVM's optimization pipeline has run.
#
# Annotations:
#   - Section headers separating module info, declarations, and functions
#   - Color-coded RC operations (alloc=blue, inc=green, dec=red, free=magenta)
#   - Color-coded COW operations (cyan)
#   - Line numbers for easy reference
#
# Environment:
#   ORI_BIN    Override path to ori binary (default: auto-detect LLVM-enabled build)
#
# Requires: ori compiler binary with LLVM support
#
# Exit codes:
#   0 = success
#   1 = compilation failed
#   2 = usage error

set -euo pipefail

# --- Locate LLVM-enabled ori binary ---
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/_common.sh"
find_ori_bin

# --- Defaults ---
RAW=0
USE_COLOR=auto
OPTIMIZED=0
FILTER_FN=""
FILE=""

# --- Parse arguments ---
while [[ $# -gt 0 ]]; do
    case $1 in
        --raw) RAW=1; shift ;;
        --color) USE_COLOR=yes; shift ;;
        --no-color) USE_COLOR=no; shift ;;
        --optimized) OPTIMIZED=1; shift ;;
        --function)
            if [[ $# -lt 2 ]]; then
                echo "Error: --function requires an argument" >&2
                exit 2
            fi
            FILTER_FN="$2"; shift 2
            ;;
        --function=*) FILTER_FN="${1#--function=}"; shift ;;
        -h|--help)
            # Print the comment block at the top of this file
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
    echo "Usage: diagnostics/ir-dump.sh [options] <file.ori>" >&2
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
tmpdir=$(mktemp -d)
trap 'rm -rf "$tmpdir"' EXIT

if [[ "$OPTIMIZED" -eq 1 ]]; then
    # Optimized IR: --emit=llvm-ir writes a .ll file
    if ! "$ORI" build "$FILE" --emit=llvm-ir -o "$tmpdir/out" 2>"$tmpdir/build_err.txt"; then
        cat "$tmpdir/build_err.txt" >&2
        echo "Error: compilation failed" >&2
        exit 1
    fi
    ir_file="$tmpdir/out.ll"
    if [[ ! -f "$ir_file" ]]; then
        echo "Error: no .ll file produced (expected $ir_file)" >&2
        exit 1
    fi
    cat "$ir_file" > "$tmpdir/ir_clean.txt"
else
    # Unoptimized IR: ORI_DEBUG_LLVM=1 dumps to stderr between markers
    if ! ORI_DEBUG_LLVM=1 "$ORI" build "$FILE" -o "$tmpdir/out" 2>"$tmpdir/ir_raw.txt"; then
        # Show build errors (strip any partial IR output)
        grep -v "^=== \(LLVM IR\|END LLVM IR\)" "$tmpdir/ir_raw.txt" >&2 || true
        echo "Error: compilation failed" >&2
        exit 1
    fi
    # Extract IR between markers, stripping the marker lines themselves
    sed -n '/^=== LLVM IR/,/^=== END LLVM IR ===/p' "$tmpdir/ir_raw.txt" | \
        sed '1d;$d' > "$tmpdir/ir_clean.txt"
fi

if [[ ! -s "$tmpdir/ir_clean.txt" ]]; then
    echo "Error: no LLVM IR captured" >&2
    exit 1
fi

# --- Filter to specific function (must run before --raw output) ---
if [[ -n "$FILTER_FN" ]]; then
    awk -v fn="$FILTER_FN" '
        /^define / && index($0, fn) { capture = 1 }
        capture { print }
        capture && /^\}/ { capture = 0 }
    ' "$tmpdir/ir_clean.txt" > "$tmpdir/ir_filtered.txt"

    if [[ ! -s "$tmpdir/ir_filtered.txt" ]]; then
        echo "Error: function '$FILTER_FN' not found in IR" >&2
        echo "Available functions:" >&2
        grep "^define " "$tmpdir/ir_clean.txt" | \
            sed 's/define [^@]*@\([^(]*\).*/  \1/' >&2
        exit 1
    fi
    mv "$tmpdir/ir_filtered.txt" "$tmpdir/ir_clean.txt"
fi

# --- Raw mode: output and exit ---
if [[ "$RAW" -eq 1 ]]; then
    cat "$tmpdir/ir_clean.txt"
    exit 0
fi

# --- Annotate: section headers, color-coded RC ops, line numbers ---
# Color codes (empty strings if --no-color)
if [[ "$USE_COLOR" == "yes" ]]; then
    C_RED='\033[0;31m'
    C_GREEN='\033[0;32m'
    C_BLUE='\033[0;34m'
    C_MAGENTA='\033[0;35m'
    C_CYAN='\033[0;36m'
    C_BOLD='\033[1m'
    C_DIM='\033[2m'
    C_NC='\033[0m'
else
    C_RED="" C_GREEN="" C_BLUE="" C_MAGENTA="" C_CYAN="" C_BOLD="" C_DIM="" C_NC=""
fi

awk \
    -v red="$C_RED" -v green="$C_GREEN" -v blue="$C_BLUE" \
    -v magenta="$C_MAGENTA" -v cyan="$C_CYAN" \
    -v bold="$C_BOLD" -v dim="$C_DIM" -v nc="$C_NC" \
'
BEGIN {
    lineno = 0
    seen_decl = 0
    seen_func = 0
    seen_meta = 0
    in_function = 0
}

# Section header: Declarations (shown once, on first declare)
/^declare / && !seen_decl {
    if (lineno > 0) printf "\n"
    printf "%s── Declarations ───────────────────────────────────%s\n", bold, nc
    seen_decl = 1
}

# Section header: Functions (shown once, on first define)
/^define / && !seen_func {
    if (lineno > 0) printf "\n"
    printf "%s── Functions ──────────────────────────────────────%s\n", bold, nc
    seen_func = 1
}

# Track function body boundaries for metadata detection
/^define / { in_function = 1 }
/^\}$/ && in_function { in_function = 0 }

# Section header: Metadata (shown once, after all functions end)
/^(attributes|!)/ && !in_function && !seen_meta {
    printf "\n"
    printf "%s── Metadata ───────────────────────────────────────%s\n", bold, nc
    seen_meta = 1
}

{
    lineno++
    line = $0

    # Color-code RC operations
    gsub(/ori_rc_alloc/, blue "ori_rc_alloc" nc, line)
    gsub(/ori_rc_inc/,   green "ori_rc_inc" nc, line)
    gsub(/ori_rc_dec/,   red "ori_rc_dec" nc, line)
    gsub(/ori_rc_free/,  magenta "ori_rc_free" nc, line)

    # Color-code COW operations
    gsub(/ori_list_[a-z_]*_cow/, cyan "&" nc, line)
    gsub(/ori_str_[a-z_]*_cow/,  cyan "&" nc, line)

    printf "%s%4d%s │ %s\n", dim, lineno, nc, line
}
' "$tmpdir/ir_clean.txt"
