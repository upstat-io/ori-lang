#!/bin/bash
# Compare the LLVM IR of two Ori source files.
#
# Usage:
#   diagnostics/ir-diff.sh [options] <a.ori> <b.ori>
#
# Options:
#   --function <name>  Compare only the named function
#   --raw              Skip IR normalization (show exact diff)
#   --optimized        Compare optimized IR (passed through to ir-dump.sh)
#   --context <n>      Number of context lines in diff (default: 3)
#   --color            Force color output (default: auto-detect terminal)
#   --no-color         Disable color output
#   -h, --help         Show this help
#
# Normalization (default, disable with --raw):
#   - Debug metadata references removed (!dbg !N)
#   - TBAA / range / alias metadata references removed
#   - Metadata definitions stripped (!0 = ..., attributes #N = ...)
#   - Block label counters normalized (then2 → then, else3 → else)
#   - Trailing whitespace stripped
#
# Output:
#   Uses delta (if available) for side-by-side colored diff, otherwise
#   falls back to diff --color (or plain diff if no color support).
#
# Environment:
#   ORI_BIN    Override path to ori binary (passed through to ir-dump.sh)
#
# Requires: ir-dump.sh (in same directory), ori compiler with LLVM support
#
# Exit codes:
#   0 = IR is identical (after normalization)
#   1 = IR differs
#   2 = usage error or compilation failure

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# --- Defaults ---
RAW=0
USE_COLOR=auto
OPTIMIZED=0
FILTER_FN=""
CONTEXT=3
FILE_A=""
FILE_B=""

# --- Parse arguments ---
while [[ $# -gt 0 ]]; do
    case $1 in
        --raw) RAW=1; shift ;;
        --color) USE_COLOR=yes; shift ;;
        --no-color) USE_COLOR=no; shift ;;
        --optimized) OPTIMIZED=1; shift ;;
        --context)
            if [[ $# -lt 2 ]]; then
                echo "Error: --context requires a number" >&2
                exit 2
            fi
            CONTEXT="$2"; shift 2
            ;;
        --context=*) CONTEXT="${1#--context=}"; shift ;;
        --function)
            if [[ $# -lt 2 ]]; then
                echo "Error: --function requires an argument" >&2
                exit 2
            fi
            FILTER_FN="$2"; shift 2
            ;;
        --function=*) FILTER_FN="${1#--function=}"; shift ;;
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
            if [[ -z "$FILE_A" ]]; then
                FILE_A="$1"
            elif [[ -z "$FILE_B" ]]; then
                FILE_B="$1"
            else
                echo "Error: too many files specified (expected exactly 2)" >&2
                exit 2
            fi
            shift
            ;;
    esac
done

if [[ -z "$FILE_A" || -z "$FILE_B" ]]; then
    echo "Error: two input files required" >&2
    echo "Usage: diagnostics/ir-diff.sh [options] <a.ori> <b.ori>" >&2
    exit 2
fi

for f in "$FILE_A" "$FILE_B"; do
    if [[ ! -f "$f" ]]; then
        echo "Error: file not found: $f" >&2
        exit 2
    fi
done

# --- Resolve color mode ---
if [[ "$USE_COLOR" == "auto" ]]; then
    if [[ -t 1 ]]; then
        USE_COLOR=yes
    else
        USE_COLOR=no
    fi
fi

# --- Build ir-dump.sh arguments ---
dump_args=(--raw)
if [[ "$OPTIMIZED" -eq 1 ]]; then
    dump_args+=(--optimized)
fi
if [[ -n "$FILTER_FN" ]]; then
    dump_args+=(--function "$FILTER_FN")
fi

# --- Capture IR from both files ---
tmpdir=$(mktemp -d)
trap 'rm -rf "$tmpdir"' EXIT

echo "Compiling $(basename "$FILE_A")..." >&2
if ! "$SCRIPT_DIR/ir-dump.sh" "${dump_args[@]}" "$FILE_A" > "$tmpdir/a_raw.ll" 2>"$tmpdir/a_err.txt"; then
    cat "$tmpdir/a_err.txt" >&2
    echo "Error: failed to compile $FILE_A" >&2
    exit 2
fi

echo "Compiling $(basename "$FILE_B")..." >&2
if ! "$SCRIPT_DIR/ir-dump.sh" "${dump_args[@]}" "$FILE_B" > "$tmpdir/b_raw.ll" 2>"$tmpdir/b_err.txt"; then
    cat "$tmpdir/b_err.txt" >&2
    echo "Error: failed to compile $FILE_B" >&2
    exit 2
fi

# --- Normalize IR (unless --raw) ---
normalize_ir() {
    sed -E \
        -e 's/, !dbg ![0-9]+//g' \
        -e 's/!dbg ![0-9]+//g' \
        -e 's/, !tbaa ![0-9]+//g' \
        -e 's/, !range ![0-9]+//g' \
        -e 's/, !alias\.scope ![0-9]+//g' \
        -e 's/, !noalias ![0-9]+//g' \
        -e '/^![0-9]+ = /d' \
        -e '/^attributes #[0-9]+ = /d' \
        -e 's/ #[0-9]+( \{)$/\1/' \
        -e 's/[[:space:]]+$//' \
    | sed -E \
        -e 's/^([a-z][a-z_.]*[a-z])[0-9]+:/\1:/' \
        -e 's/label %([a-z][a-z_.]*[a-z])[0-9]+/label %\1/g' \
        -e 's/\[([^]]*), %([a-z][a-z_.]*[a-z])[0-9]+ \]/[\1, %\2 ]/g'
}

if [[ "$RAW" -eq 1 ]]; then
    cp "$tmpdir/a_raw.ll" "$tmpdir/a.ll"
    cp "$tmpdir/b_raw.ll" "$tmpdir/b.ll"
else
    normalize_ir < "$tmpdir/a_raw.ll" > "$tmpdir/a.ll"
    normalize_ir < "$tmpdir/b_raw.ll" > "$tmpdir/b.ll"
fi

# --- Diff ---
label_a="$(basename "$FILE_A")"
label_b="$(basename "$FILE_B")"

if command -v delta &>/dev/null && [[ "$USE_COLOR" == "yes" ]]; then
    # delta provides excellent side-by-side colored output
    delta --file-modified-label "" \
        --hunk-header-style "line-number" \
        --paging never \
        "$tmpdir/a.ll" "$tmpdir/b.ll" \
        --file-renamed-show-old-path \
        -- --label "$label_a" --label "$label_b" \
        || exit_code=$?
elif [[ "$USE_COLOR" == "yes" ]]; then
    diff --color=always -u "$tmpdir/a.ll" "$tmpdir/b.ll" \
        --label "$label_a" --label "$label_b" \
        -U "$CONTEXT" \
        || exit_code=$?
else
    diff -u "$tmpdir/a.ll" "$tmpdir/b.ll" \
        --label "$label_a" --label "$label_b" \
        -U "$CONTEXT" \
        || exit_code=$?
fi

# diff exits 0 if identical, 1 if different, 2 if trouble
exit_code=${exit_code:-0}
if [[ "$exit_code" -eq 0 ]]; then
    echo "IR is identical (after normalization)." >&2
elif [[ "$exit_code" -ge 2 ]]; then
    echo "Error: diff command failed" >&2
    exit 2
fi

exit "$exit_code"
