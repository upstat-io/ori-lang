#!/bin/bash
# Run AOT-compiled Ori programs under Valgrind to detect memory errors.
#
# Usage:
#   diagnostics/valgrind-aot.sh [options] [file.ori|directory ...]
#
# Options:
#   --journeys         Run all code journey .ori files in plans/code-journeys/
#   --no-color         Disable color output
#   --color            Force color output (default: auto-detect terminal)
#   -h, --help         Show this help
#
# When no files are given, runs all .ori files in tests/valgrind/.
#
# Checks for:
#   - Use-after-free (invalid reads/writes)
#   - Double-free
#   - Memory leaks (via Valgrind, not ORI_CHECK_LEAKS)
#   - Buffer overflows
#
# Environment:
#   ORI_BIN    Override path to ori binary (must have LLVM support)
#
# Exit codes:
#   0 = all tests clean
#   1 = memory errors detected
#   2 = usage error or missing dependency

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/_common.sh"

# --- Defaults ---
USE_COLOR=auto
FILES=()
USE_JOURNEYS=0

# --- Parse arguments ---
while [[ $# -gt 0 ]]; do
    case $1 in
        --color) USE_COLOR=yes; shift ;;
        --no-color) USE_COLOR=no; shift ;;
        --journeys) USE_JOURNEYS=1; shift ;;
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
            if [[ -d "$1" ]]; then
                # Expand directory to all .ori files within it
                while IFS= read -r -d '' f; do
                    FILES+=("$f")
                done < <(find "$1" -name '*.ori' -type f -print0 | sort -z)
                if [[ ${#FILES[@]} -eq 0 ]]; then
                    echo "Error: no .ori files found in directory: $1" >&2
                    exit 2
                fi
            elif [[ -f "$1" ]]; then
                FILES+=("$1")
            else
                echo "Error: not a file or directory: $1" >&2
                exit 2
            fi
            shift
            ;;
    esac
done

# --- Locate LLVM-enabled binary ---
find_ori_bin

# --- Check Valgrind is installed ---
if ! command -v valgrind &>/dev/null; then
    echo "Error: valgrind is not installed" >&2
    echo "Install with: sudo apt install valgrind (Debian/Ubuntu)" >&2
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
    SYM_SKIP="${C_DIM}SKIP${C_NC}"
else
    C_RED="" C_GREEN="" C_YELLOW="" C_BOLD="" C_DIM="" C_NC=""
    SYM_PASS="PASS"
    SYM_FAIL="FAIL"
    SYM_SKIP="SKIP"
fi

# --- Collect code journey files if --journeys ---
if [[ $USE_JOURNEYS -eq 1 ]]; then
    ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
    journey_dir="$ROOT_DIR/plans/code-journeys"
    if [[ ! -d "$journey_dir" ]]; then
        echo "Error: plans/code-journeys/ directory not found" >&2
        exit 2
    fi
    for f in "$journey_dir"/*.ori; do
        [[ -f "$f" ]] || continue
        FILES+=("$f")
    done
    if [[ ${#FILES[@]} -eq 0 ]]; then
        echo "Error: no .ori files found in $journey_dir" >&2
        exit 2
    fi
fi

# --- Collect files ---
if [[ ${#FILES[@]} -eq 0 ]]; then
    ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
    test_dir="$ROOT_DIR/tests/valgrind"
    if [[ ! -d "$test_dir" ]]; then
        echo "No tests/valgrind/ directory and no files specified." >&2
        echo "Usage: diagnostics/valgrind-aot.sh [options] [file.ori ...]" >&2
        exit 2
    fi
    for f in "$test_dir"/*.ori; do
        [[ -f "$f" ]] || continue
        FILES+=("$f")
    done
    if [[ ${#FILES[@]} -eq 0 ]]; then
        echo "No .ori files found in $test_dir" >&2
        exit 2
    fi
fi

# --- Setup ---
tmpdir=$(mktemp -d)
trap 'rm -rf "$tmpdir"' EXIT

PASS_COUNT=0
FAIL_COUNT=0
SKIP_COUNT=0
ERRORS=()

# --- Run one file ---
run_one() {
    local ori_file="$1"
    local name
    name=$(basename "$ori_file" .ori)
    local binary="$tmpdir/$name"

    # Compile
    if ! "$ORI" build "$ori_file" -o "$binary" 2>"$tmpdir/compile_${name}.log"; then
        printf "  %b  %s (AOT compile failed — run 'ori build %s' to see errors)\n" "$SYM_SKIP" "$name" "$ori_file"
        SKIP_COUNT=$((SKIP_COUNT + 1))
        return
    fi

    # Run under Valgrind
    local val_log="$tmpdir/valgrind_${name}.log"
    local exit_code
    exit_code=$(ORI_CHECK_LEAKS=1 valgrind \
        --leak-check=full \
        --show-leak-kinds=all \
        --errors-for-leak-kinds=definite \
        --error-exitcode=42 \
        --log-file="$val_log" \
        "$binary" >/dev/null 2>&1; echo $?) || true

    if [[ "$exit_code" -eq 0 ]]; then
        printf "  %b  %s\n" "$SYM_PASS" "$name"
        PASS_COUNT=$((PASS_COUNT + 1))
    elif [[ "$exit_code" -eq 42 ]]; then
        printf "  %b  %s (memory errors)\n" "$SYM_FAIL" "$name"
        grep -E "(definitely lost|Invalid|ERROR SUMMARY)" "$val_log" | sed 's/^/        /'
        FAIL_COUNT=$((FAIL_COUNT + 1))
        ERRORS+=("$name")
    else
        printf "  %b  %s (exit code %d)\n" "$SYM_FAIL" "$name" "$exit_code"
        FAIL_COUNT=$((FAIL_COUNT + 1))
        ERRORS+=("$name")
    fi
}

# --- Main ---
printf "${C_BOLD}=== Valgrind AOT Memory Check ===${C_NC}\n\n"

for f in "${FILES[@]}"; do
    run_one "$f"
done

echo ""
printf "${C_BOLD}=== Results: %d passed, %d failed, %d skipped ===${C_NC}\n" \
    "$PASS_COUNT" "$FAIL_COUNT" "$SKIP_COUNT"

if [[ ${#ERRORS[@]} -gt 0 ]]; then
    printf "Failures: %s\n" "${ERRORS[*]}"
    exit 1
fi
