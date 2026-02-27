#!/bin/bash
# Static analysis of LLVM IR for RC, COW, and ABI correctness.
#
# Usage:
#   diagnostics/codegen-audit.sh [options] <file.ori>
#
# Options:
#   --strict           Pessimistic mode: treat COW as always-freeing, elevate
#                      warnings to errors, track function pointer params as RC-managed
#   --function <name>  Only audit functions whose LLVM name contains <name>
#   --no-color         Disable color output
#   --color            Force color output (default: auto-detect terminal)
#   -h, --help         Show this help
#
# Performs three categories of static analysis on the emitted LLVM IR:
#
#   1. RC Balance      — Tracks ori_rc_alloc/inc/dec/free lifecycle per function.
#                        Detects leaks (alloc without dec) and double-frees
#                        (dec after COW consumption).
#
#   2. COW Correctness — Verifies COW operation sequencing:
#                        - No pointer reuse after COW call (invalidated by realloc)
#                        - No ori_rc_dec before COW call (freed pointer passed in)
#
#   3. ABI Conformance — Checks calling conventions:
#                        - No large aggregate loads (>16B, FastISel JIT bug)
#                        - Runtime function arg counts match declarations
#                        - No invoke on nounwind functions
#
# The analysis runs in-pipeline via ORI_AUDIT_CODEGEN=1, walking the live
# inkwell IR objects (not parsed textual IR).
#
# Environment:
#   ORI_BIN    Override path to ori binary
#
# Exit codes:
#   0 = clean (no findings)
#   1 = findings detected (errors or warnings)
#   2 = usage error or compilation failure

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/_common.sh"
find_ori_bin

# --- Defaults ---
STRICT=0
FUNCTION_FILTER=""
USE_COLOR=auto
FILE=""

# --- Parse arguments ---
while [[ $# -gt 0 ]]; do
    case $1 in
        --strict) STRICT=1; shift ;;
        --function)
            if [[ $# -lt 2 ]]; then
                echo "Error: --function requires an argument" >&2
                exit 2
            fi
            FUNCTION_FILTER="$2"; shift 2
            ;;
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
    echo "Usage: diagnostics/codegen-audit.sh [options] <file.ori>" >&2
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

# --- Build environment ---
export ORI_AUDIT_CODEGEN=1
if [[ "$STRICT" -eq 1 ]]; then
    export ORI_AUDIT_STRICT=1
fi
if [[ -n "$FUNCTION_FILTER" ]]; then
    export ORI_AUDIT_FUNCTION="$FUNCTION_FILTER"
fi

# --- Header ---
printf "${C_BOLD}=== Codegen Audit: %s ===${C_NC}\n" "$(basename "$FILE")"
if [[ "$STRICT" -eq 1 ]]; then
    printf "${C_YELLOW}Mode: strict (pessimistic)${C_NC}\n"
fi
if [[ -n "$FUNCTION_FILTER" ]]; then
    printf "${C_DIM}Filter: functions matching '%s'${C_NC}\n" "$FUNCTION_FILTER"
fi
echo ""

# --- Run compilation with audit enabled ---
# The audit output goes to stderr; stdout is the normal compilation output.
# We capture stderr separately to parse the audit results.
tmpdir=$(mktemp -d)
trap 'rm -rf "$tmpdir"' EXIT

# Try `ori build` first (AOT path), fall back to `ori check` (JIT path)
printf "${C_DIM}Compiling...${C_NC}\n"
if "$ORI" build "$FILE" > "$tmpdir/stdout.txt" 2> "$tmpdir/stderr.txt"; then
    COMPILE_OK=1
    # Clean up the built binary
    BINARY="${FILE%.ori}"
    [[ -f "$BINARY" ]] && rm -f "$BINARY"
else
    # Check if compilation itself failed (vs just audit findings)
    if grep -q "^codegen audit:" "$tmpdir/stderr.txt"; then
        COMPILE_OK=1  # Compilation succeeded, audit found issues
    else
        COMPILE_OK=0
    fi
fi

# --- Parse and display results ---
if [[ "$COMPILE_OK" -eq 0 ]]; then
    printf "${C_RED}Compilation failed:${C_NC}\n"
    cat "$tmpdir/stderr.txt"
    exit 2
fi

# Extract audit lines from stderr
AUDIT_LINES=$(grep "^codegen audit:" "$tmpdir/stderr.txt" 2>/dev/null || true)

if [[ -z "$AUDIT_LINES" ]]; then
    printf "${C_GREEN}No findings — IR is clean.${C_NC}\n"
    exit 0
fi

# Colorize output
echo "$AUDIT_LINES" | while IFS= read -r line; do
    if [[ "$line" == *"error:"* ]]; then
        printf "${C_RED}%s${C_NC}\n" "$line"
    elif [[ "$line" == *"warning:"* ]]; then
        printf "${C_YELLOW}%s${C_NC}\n" "$line"
    elif [[ "$line" == *"summary:"* ]]; then
        printf "\n${C_BOLD}%s${C_NC}\n" "$line"
    else
        printf "%s\n" "$line"
    fi
done

# Determine exit code from summary
ERROR_COUNT=$(echo "$AUDIT_LINES" | grep "summary:" | grep -oP '\d+ error' | grep -oP '^\d+' || echo "0")
WARNING_COUNT=$(echo "$AUDIT_LINES" | grep "summary:" | grep -oP '\d+ warning' | grep -oP '^\d+' || echo "0")

if [[ "$ERROR_COUNT" -gt 0 ]] || [[ "$WARNING_COUNT" -gt 0 ]]; then
    exit 1
fi
exit 0
