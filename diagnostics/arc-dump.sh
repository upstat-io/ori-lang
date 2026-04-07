#!/bin/bash
# Dump annotated ARC IR for an Ori source file.
#
# Usage:
#   diagnostics/arc-dump.sh [options] <file.ori>
#
# Options:
#   --raw              Output raw ARC IR without annotations or color
#   --color            Force color output (default: auto-detect terminal)
#   --no-color         Disable color output
#   --function <name>  Show only the named function (matches "fn @<name>")
#   -h, --help         Show this help
#
# Captures the typed ARC IR via ORI_DUMP_AFTER_ARC=1 — the IR after
# CanExpr lowering but before AIMS RC emission. This is the level at
# which take-projects, alias chains, block params (phi merges), and
# Project / Construct / Apply instructions are visible. Use it to
# debug AIMS pipeline issues: lineage analysis, RC placement, drop
# hints, alias-class membership, bypass-safe regions.
#
# Annotations (default mode):
#   - Section headers separating functions
#   - Color-coded RC operations (RcInc=green, RcDec=red)
#   - Color-coded ownership transfers (Construct/Project=blue)
#   - Line numbers for easy reference
#
# For LLVM IR (post-codegen) use ir-dump.sh instead.
#
# Environment:
#   ORI_BIN    Override path to ori binary (default: auto-detect LLVM-enabled build)
#
# Requires: ori compiler binary (LLVM support not required, but the
# default `cargo b` build includes it)
#
# Exit codes:
#   0 = success
#   1 = compilation failed and no IR was captured
#   2 = usage error

set -euo pipefail

# --- Locate ori binary ---
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/_common.sh"
find_any_ori_bin
ORI="$ORI_INTERP"

# --- Defaults ---
RAW=0
USE_COLOR=auto
FILTER_FN=""
FILE=""

# --- Parse arguments ---
while [[ $# -gt 0 ]]; do
    case $1 in
        --raw) RAW=1; shift ;;
        --color) USE_COLOR=yes; shift ;;
        --no-color) USE_COLOR=no; shift ;;
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
    echo "Usage: diagnostics/arc-dump.sh [options] <file.ori>" >&2
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

# ORI_DUMP_AFTER_ARC=1 emits the ARC IR to stderr between markers.
# The dump runs before LLVM codegen, so the IR is captured even when a
# subsequent codegen step fails (e.g., LLVM IR verification error).
build_exit=0
ORI_DUMP_AFTER_ARC=1 "$ORI" build "$FILE" -o "$tmpdir/out" 2>"$tmpdir/arc_raw.txt" || build_exit=$?

# Extract IR between markers, stripping the marker lines themselves.
sed -n '/^=== ARC IR after lowering/,/^=== END ARC IR ===/p' "$tmpdir/arc_raw.txt" \
    | sed '1d;$d' > "$tmpdir/arc_clean.txt"

if [[ ! -s "$tmpdir/arc_clean.txt" ]]; then
    if [[ "$build_exit" -ne 0 ]]; then
        # No IR captured and build failed — show the build error
        grep -v '^=== \(ARC IR\|END ARC IR\)' "$tmpdir/arc_raw.txt" >&2 || true
        echo "Error: compilation failed and no ARC IR was captured" >&2
        exit 1
    fi
    echo "Error: no ARC IR captured (is ORI_DUMP_AFTER_ARC supported by this build?)" >&2
    exit 1
fi

if [[ "$build_exit" -ne 0 ]]; then
    # IR was captured before the failure — show it with a warning
    echo "Warning: build failed (exit $build_exit) but ARC IR was captured before the error" >&2
    grep -v '^=== \(ARC IR\|END ARC IR\)' "$tmpdir/arc_raw.txt" \
        | grep -v '^[[:space:]]*$' \
        | tail -20 >&2 || true
fi

# --- Filter to specific function ---
if [[ -n "$FILTER_FN" ]]; then
    # ARC IR functions look like "fn @main() -> int [entry: bb0]" — capture
    # from a matching function header to the next blank line or next "fn @"
    # header.
    awk -v fn="$FILTER_FN" '
        /^fn @/ {
            if (index($0, "@" fn "(")) { capture = 1 } else { capture = 0 }
        }
        capture { print }
    ' "$tmpdir/arc_clean.txt" > "$tmpdir/arc_filtered.txt"

    if [[ ! -s "$tmpdir/arc_filtered.txt" ]]; then
        echo "Error: function '$FILTER_FN' not found in ARC IR" >&2
        echo "Available functions:" >&2
        grep '^fn @' "$tmpdir/arc_clean.txt" \
            | sed 's/^fn @\([^(]*\).*/  \1/' >&2
        exit 1
    fi
    mv "$tmpdir/arc_filtered.txt" "$tmpdir/arc_clean.txt"
fi

# --- Raw mode: output and exit ---
if [[ "$RAW" -eq 1 ]]; then
    cat "$tmpdir/arc_clean.txt"
    exit 0
fi

# --- Annotate: section headers, color-coded RC ops, line numbers ---
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
BEGIN { lineno = 0; first_fn = 1 }

# Print a separator before each function header (except the first)
/^fn @/ {
    if (!first_fn) printf "\n"
    first_fn = 0
    lineno++
    line = $0
    gsub(/^fn @/, bold "fn @", line)
    gsub(/$/,     nc, line)
    printf "%s%4d%s │ %s\n", dim, lineno, nc, line
    next
}

{
    lineno++
    line = $0

    # Color-code RC operations
    gsub(/RcInc/,    green "RcInc" nc, line)
    gsub(/RcDec/,    red "RcDec" nc, line)

    # Color-code ownership-transferring instructions
    gsub(/= Construct/, "= " blue "Construct" nc, line)
    gsub(/= Project/,   "= " blue "Project" nc, line)
    gsub(/= PartialApply/, "= " blue "PartialApply" nc, line)

    # Color-code Apply / Invoke (calls)
    gsub(/= Apply/,         "= " cyan "Apply" nc, line)
    gsub(/= Invoke /,       "= " cyan "Invoke" nc " ", line)
    gsub(/= ApplyIndirect/, "= " cyan "ApplyIndirect" nc, line)

    # Highlight Switch / Branch / Jump terminators
    gsub(/^    Switch/,        "    " magenta "Switch" nc, line)
    gsub(/^    Branch/,        "    " magenta "Branch" nc, line)
    gsub(/^    Jump/,          "    " magenta "Jump" nc, line)
    gsub(/^    Return/,        "    " magenta "Return" nc, line)
    gsub(/^    Resume/,        "    " magenta "Resume" nc, line)
    gsub(/^    Unreachable/,   "    " magenta "Unreachable" nc, line)

    printf "%s%4d%s │ %s\n", dim, lineno, nc, line
}
' "$tmpdir/arc_clean.txt"
