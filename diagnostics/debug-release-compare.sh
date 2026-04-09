#!/bin/bash
# debug-release-compare.sh — compare debug vs release build output
#
# Compiles and runs an Ori program through both target/debug/ori and
# target/release/ori, comparing exit codes and stdout. Catches FastISel-only
# bugs and optimization-dependent codegen divergences.
#
# Usage:
#   diagnostics/debug-release-compare.sh [options] <file.ori>
#
# Options:
#   --verbose     Show LLVM IR diff on mismatch
#   --color       Force color output
#   --no-color    Disable color output
#   -h, --help    Show this help
#
# Exit codes:
#   0  Both builds produce matching output
#   1  Mismatch detected (different exit codes or stdout)
#   2  Usage or infrastructure error
#
# Prerequisites:
#   Both debug and release binaries must exist with LLVM support:
#     cargo b            # debug
#     cargo b --release  # release

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/_common.sh"

# --- Defaults ---
VERBOSE=0
USE_COLOR=auto
FILE=""

# --- Argument parsing ---
while [[ $# -gt 0 ]]; do
    case $1 in
        -h|--help)
            sed -n '2,/^$/{ s/^# \?//; p }' "$0"
            exit 0
            ;;
        --verbose)
            VERBOSE=1
            shift
            ;;
        --color)
            USE_COLOR=yes
            shift
            ;;
        --no-color)
            USE_COLOR=no
            shift
            ;;
        -*)
            echo "Error: unknown option: $1" >&2
            echo "Run with --help for usage" >&2
            exit 2
            ;;
        *)
            if [[ -n "$FILE" ]]; then
                echo "Error: multiple files specified" >&2
                exit 2
            fi
            FILE="$1"
            shift
            ;;
    esac
done

if [[ -z "$FILE" ]]; then
    echo "Error: no input file specified" >&2
    echo "Usage: $0 [options] <file.ori>" >&2
    exit 2
fi

if [[ ! -f "$FILE" ]]; then
    echo "Error: file not found: $FILE" >&2
    exit 2
fi

# --- Validate both builds ---
require_both_builds

# --- Color setup ---
if [[ "$USE_COLOR" == "auto" ]]; then
    if [[ -t 1 ]]; then
        USE_COLOR=yes
    else
        USE_COLOR=no
    fi
fi

if [[ "$USE_COLOR" == "yes" ]]; then
    C_RED='\033[0;31m'
    C_GREEN='\033[0;32m'
    C_YELLOW='\033[0;33m'
    C_BOLD='\033[1m'
    C_DIM='\033[2m'
    C_NC='\033[0m'
else
    C_RED='' C_GREEN='' C_YELLOW='' C_BOLD='' C_DIM='' C_NC=''
fi

# --- Temp directory ---
tmpdir=$(mktemp -d)
trap 'rm -rf "$tmpdir"' EXIT

# --- Step 1: Debug build ---
echo -e "${C_BOLD}=== Debug build ===${C_NC}"
echo -e "${C_DIM}Binary: $ORI_DEBUG${C_NC}"

debug_exit=0
"$ORI_DEBUG" build "$FILE" -o "$tmpdir/debug_bin" 2>"$tmpdir/debug_build_err.txt" || debug_exit=$?

if [[ $debug_exit -ne 0 ]]; then
    echo -e "${C_RED}Debug build failed (exit $debug_exit)${C_NC}"
    cat "$tmpdir/debug_build_err.txt"
    exit 1
fi

debug_run_exit=0
"$tmpdir/debug_bin" >"$tmpdir/debug_stdout.txt" 2>"$tmpdir/debug_stderr.txt" || debug_run_exit=$?

echo -e "  Exit: $debug_run_exit"

# --- Step 2: Release build ---
echo -e "${C_BOLD}=== Release build ===${C_NC}"
echo -e "${C_DIM}Binary: $ORI_RELEASE${C_NC}"

release_exit=0
"$ORI_RELEASE" build "$FILE" -o "$tmpdir/release_bin" 2>"$tmpdir/release_build_err.txt" || release_exit=$?

if [[ $release_exit -ne 0 ]]; then
    echo -e "${C_RED}Release build failed (exit $release_exit)${C_NC}"
    cat "$tmpdir/release_build_err.txt"
    exit 1
fi

release_run_exit=0
"$tmpdir/release_bin" >"$tmpdir/release_stdout.txt" 2>"$tmpdir/release_stderr.txt" || release_run_exit=$?

echo -e "  Exit: $release_run_exit"

# --- Step 3: Compare ---
echo ""
has_mismatch=0

# Compare exit codes
if [[ $debug_run_exit -ne $release_run_exit ]]; then
    echo -e "${C_RED}${C_BOLD}EXIT CODE MISMATCH${C_NC}"
    echo -e "  Debug:   $debug_run_exit"
    echo -e "  Release: $release_run_exit"
    has_mismatch=1
fi

# Compare stdout
debug_stdout=$(cat "$tmpdir/debug_stdout.txt")
release_stdout=$(cat "$tmpdir/release_stdout.txt")

if [[ "$debug_stdout" != "$release_stdout" ]]; then
    echo -e "${C_RED}${C_BOLD}STDOUT MISMATCH${C_NC}"
    echo -e "${C_DIM}--- debug${C_NC}"
    echo -e "${C_DIM}+++ release${C_NC}"
    diff --color=never "$tmpdir/debug_stdout.txt" "$tmpdir/release_stdout.txt" || true
    has_mismatch=1
fi

# --- Step 4: Auto-diagnostics on mismatch ---
if [[ $has_mismatch -eq 1 ]]; then
    echo ""
    echo -e "${C_YELLOW}${C_BOLD}=== Auto-diagnostics ===${C_NC}"

    # Dump LLVM IR from both builds
    echo -e "${C_DIM}Dumping debug LLVM IR...${C_NC}"
    ORI_BIN="$ORI_DEBUG" "$SCRIPT_DIR/ir-dump.sh" --raw "$FILE" > "$tmpdir/debug_ir.txt" 2>/dev/null || true

    echo -e "${C_DIM}Dumping release LLVM IR...${C_NC}"
    ORI_BIN="$ORI_RELEASE" "$SCRIPT_DIR/ir-dump.sh" --raw "$FILE" > "$tmpdir/release_ir.txt" 2>/dev/null || true

    if [[ -s "$tmpdir/debug_ir.txt" ]] && [[ -s "$tmpdir/release_ir.txt" ]]; then
        ir_diff=$(diff "$tmpdir/debug_ir.txt" "$tmpdir/release_ir.txt" 2>/dev/null || true)
        if [[ -n "$ir_diff" ]]; then
            ir_diff_lines=$(echo "$ir_diff" | wc -l)
            echo -e "  LLVM IR differs (${ir_diff_lines} lines)"
            if [[ $VERBOSE -eq 1 ]]; then
                echo -e "${C_DIM}--- debug IR${C_NC}"
                echo -e "${C_DIM}+++ release IR${C_NC}"
                echo "$ir_diff"
            else
                echo -e "  ${C_DIM}(use --verbose to see full IR diff)${C_NC}"
            fi
        else
            echo -e "  LLVM IR is identical (behavioral difference may be in optimization)"
        fi
    else
        echo -e "  ${C_YELLOW}Could not dump LLVM IR for comparison${C_NC}"
    fi

    # Also show RC stats from both builds
    if [[ $VERBOSE -eq 1 ]]; then
        echo ""
        echo -e "${C_DIM}Debug RC stats:${C_NC}"
        color_flag=""
        [[ "$USE_COLOR" == "yes" ]] && color_flag="--color"
        [[ "$USE_COLOR" == "no" ]] && color_flag="--no-color"
        ORI_BIN="$ORI_DEBUG" "$SCRIPT_DIR/rc-stats.sh" $color_flag "$FILE" 2>/dev/null | sed 's/^/  │ /' || true

        echo -e "${C_DIM}Release RC stats:${C_NC}"
        ORI_BIN="$ORI_RELEASE" "$SCRIPT_DIR/rc-stats.sh" $color_flag "$FILE" 2>/dev/null | sed 's/^/  │ /' || true
    fi
fi

# --- Summary ---
echo ""
if [[ $has_mismatch -eq 0 ]]; then
    echo -e "${C_GREEN}${C_BOLD}MATCH${C_NC} — debug and release builds produce identical output"
else
    echo -e "${C_RED}${C_BOLD}MISMATCH${C_NC} — debug and release builds diverge"
fi

exit $has_mismatch
