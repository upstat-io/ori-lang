#!/bin/bash
# AIMS Pipeline Comparison: Compare old ARC pipeline vs AIMS pipeline.
#
# Usage:
#   diagnostics/aims-compare.sh [options] [test-path]
#
# Options:
#   -v, --verbose      Show all test results, not just differences
#   --rc-only          Skip behavioral comparison, only compare RC counts
#   --behavioral-only  Skip RC count comparison, only check behavioral equivalence
#   --release          Build in release mode (slower build, faster tests)
#   --no-color         Disable color output
#   --color            Force color output
#   -h, --help         Show this help
#
# Builds old (no aims feature) and AIMS (--features aims) binaries sequentially,
# capturing ARC IR dumps and program output from each before rebuilding.
#
#   Pass 1: Behavioral equivalence — AOT-compiled @main programs must produce
#           identical output and exit codes. Hard failure on any difference.
#   Pass 2: RC operation count — compare RcInc/RcDec counts in ARC IR dumps.
#           Improvements logged, regressions flagged.
#
# The script compares LLVM/AOT behavior only. The interpreter does not use
# the ARC pipeline, so `ori test` (interpreter) results are AIMS-independent.
#
# Exit codes:
#   0 = No behavioral regressions, RC comparison complete
#   1 = Behavioral regression found (AIMS produces different output)
#   2 = Infrastructure error (build failure, missing binary)

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# --- Defaults ---
TEST_PATH="tests/spec/"
VERBOSE=0
RC_ONLY=0
BEHAVIORAL_ONLY=0
BUILD_PROFILE="debug"
BUILD_FLAGS=""
USE_COLOR=auto

# --- Parse arguments ---
while [[ $# -gt 0 ]]; do
    case $1 in
        -v|--verbose) VERBOSE=1; shift ;;
        --rc-only) RC_ONLY=1; shift ;;
        --behavioral-only) BEHAVIORAL_ONLY=1; shift ;;
        --release) BUILD_PROFILE="release"; BUILD_FLAGS="--release"; shift ;;
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
            if [[ -e "$1" || -e "$ROOT_DIR/$1" ]]; then
                TEST_PATH="$1"
            else
                echo "Error: path not found: $1" >&2
                exit 2
            fi
            shift
            ;;
    esac
done

# --- Color codes ---
if [[ "$USE_COLOR" == "auto" ]]; then
    [[ -t 1 ]] && USE_COLOR=yes || USE_COLOR=no
fi
if [[ "$USE_COLOR" == "yes" ]]; then
    C_RED='\033[0;31m'
    C_GREEN='\033[0;32m'
    C_YELLOW='\033[0;33m'
    C_CYAN='\033[0;36m'
    C_BOLD='\033[1m'
    C_DIM='\033[2m'
    C_NC='\033[0m'
else
    C_RED="" C_GREEN="" C_YELLOW="" C_CYAN="" C_BOLD="" C_DIM="" C_NC=""
fi

# --- Data directory ---
DATA_DIR="$ROOT_DIR/build/aims-compare"
OLD_DATA="$DATA_DIR/old"
AIMS_DATA="$DATA_DIR/aims"
mkdir -p "$OLD_DATA" "$AIMS_DATA"

# --- Temp files ---
TMP_OUTPUT=$(mktemp)
TMP_ARC=$(mktemp)
cleanup() { rm -f "$TMP_OUTPUT" "$TMP_ARC"; }
trap cleanup EXIT

# --- Resolve test path ---
if [[ "$TEST_PATH" != /* ]]; then
    TEST_PATH="$ROOT_DIR/$TEST_PATH"
fi

ORI_BIN="$ROOT_DIR/target/$BUILD_PROFILE/ori"

find_all_ori_files() {
    local search_path="$1"
    if [[ -f "$search_path" ]]; then
        echo "$search_path"
    else
        find "$search_path" -name "*.ori" -type f 2>/dev/null | sort
    fi
}

count_rc_ops() {
    local file="$1"
    local inc dec
    inc=$(grep -c "RcInc" "$file" 2>/dev/null) || inc=0
    dec=$(grep -c "RcDec" "$file" 2>/dev/null) || dec=0
    echo $((inc + dec))
}

# Sanitize a path into a flat filename for data storage
path_key() {
    echo "$1" | sed "s|$ROOT_DIR/||; s|/|__|g"
}

# =============================================================================
# Phase 1: Build old pipeline, capture data
# =============================================================================

echo -e "${C_BOLD}Phase 1: Building old pipeline (no aims feature)...${C_NC}"
if ! cargo build -p oric $BUILD_FLAGS 2>&1 | tail -1; then
    echo "Error: old pipeline build failed" >&2
    exit 2
fi

echo -e "${C_DIM}Capturing old pipeline ARC IR dumps...${C_NC}"
ALL_ORI_FILES=$(find_all_ori_files "$TEST_PATH")
old_captured=0

while IFS= read -r test; do
    [[ -z "$test" ]] && continue
    key=$(path_key "$test")

    # Capture ARC IR dump (exit code may be non-zero for test-only files without @main)
    ORI_DUMP_AFTER_ARC=1 "$ORI_BIN" build "$test" -o /dev/null > /dev/null 2> "$OLD_DATA/$key.arc" || true
    if [[ -s "$OLD_DATA/$key.arc" ]]; then
        old_captured=$((old_captured + 1))
    else
        rm -f "$OLD_DATA/$key.arc"
    fi

    # Capture behavioral output for @main programs
    if grep -q '@main' "$test" 2>/dev/null; then
        if "$ORI_BIN" build "$test" -o "$TMP_OUTPUT.bin" 2>/dev/null; then
            old_exit=0
            "$TMP_OUTPUT.bin" > "$OLD_DATA/$key.out" 2>&1 || old_exit=$?
            echo "$old_exit" > "$OLD_DATA/$key.exit"
            rm -f "$TMP_OUTPUT.bin"
        fi
    fi
done <<< "$ALL_ORI_FILES"

echo -e "${C_DIM}Captured $old_captured files${C_NC}"
echo ""

# =============================================================================
# Phase 2: Build AIMS pipeline, capture data and compare
# =============================================================================

echo -e "${C_BOLD}Phase 2: Building AIMS pipeline (--features aims)...${C_NC}"
if ! cargo build -p oric $BUILD_FLAGS --features aims 2>&1 | tail -1; then
    echo "Error: AIMS pipeline build failed" >&2
    exit 2
fi
echo ""

# --- Behavioral Equivalence ---
semantic_failures=0
semantic_matches=0
semantic_skipped=0

if [[ "$RC_ONLY" -eq 0 ]]; then
    echo -e "${C_BOLD}=== Pass 1: Behavioral Equivalence (AOT @main programs) ===${C_NC}"
    echo ""

    while IFS= read -r test; do
        [[ -z "$test" ]] && continue
        grep -q '@main' "$test" 2>/dev/null || continue
        rel_path="${test#$ROOT_DIR/}"
        key=$(path_key "$test")

        # Check if old pipeline had output
        if [[ ! -f "$OLD_DATA/$key.out" ]]; then
            if [[ "$VERBOSE" -eq 1 ]]; then
                echo -e "  ${C_DIM}SKIP (no old output): $rel_path${C_NC}"
            fi
            semantic_skipped=$((semantic_skipped + 1))
            continue
        fi

        # Build with AIMS pipeline
        if ! "$ORI_BIN" build "$test" -o "$TMP_OUTPUT.bin" 2>/dev/null; then
            echo -e "  ${C_RED}REGRESSION (AIMS build failed, old succeeded): $rel_path${C_NC}"
            semantic_failures=$((semantic_failures + 1))
            continue
        fi

        # Run and compare
        aims_exit=0
        "$TMP_OUTPUT.bin" > "$TMP_OUTPUT" 2>&1 || aims_exit=$?
        rm -f "$TMP_OUTPUT.bin"

        old_exit=$(cat "$OLD_DATA/$key.exit" 2>/dev/null || echo "0")

        if [[ "$old_exit" != "$aims_exit" ]]; then
            echo -e "  ${C_RED}REGRESSION (exit code: old=$old_exit, aims=$aims_exit): $rel_path${C_NC}"
            semantic_failures=$((semantic_failures + 1))
        elif ! diff "$OLD_DATA/$key.out" "$TMP_OUTPUT" > /dev/null 2>&1; then
            echo -e "  ${C_RED}REGRESSION (different output): $rel_path${C_NC}"
            semantic_failures=$((semantic_failures + 1))
        else
            if [[ "$VERBOSE" -eq 1 ]]; then
                echo -e "  ${C_GREEN}MATCH: $rel_path${C_NC}"
            fi
            semantic_matches=$((semantic_matches + 1))
        fi
    done <<< "$ALL_ORI_FILES"

    echo ""
    echo -e "${C_BOLD}Behavioral: ${C_GREEN}$semantic_matches matches${C_NC}, " \
         "${C_RED}$semantic_failures regressions${C_NC}, " \
         "${C_DIM}$semantic_skipped skipped${C_NC}"
    echo ""
fi

# --- RC Operation Count Comparison ---
rc_improvements=0
rc_regressions=0
rc_matches=0
rc_skipped=0
total_old_rc=0
total_aims_rc=0
declare -a RC_REGRESSION_DETAILS=()
declare -a RC_IMPROVEMENT_DETAILS=()

if [[ "$BEHAVIORAL_ONLY" -eq 0 ]]; then
    echo -e "${C_BOLD}=== Pass 2: RC Operation Count Comparison ===${C_NC}"
    echo ""

    while IFS= read -r test; do
        [[ -z "$test" ]] && continue
        rel_path="${test#$ROOT_DIR/}"
        key=$(path_key "$test")

        # Check old data exists
        if [[ ! -f "$OLD_DATA/$key.arc" ]]; then
            if [[ "$VERBOSE" -eq 1 ]]; then
                echo -e "  ${C_DIM}SKIP (no old ARC data): $rel_path${C_NC}"
            fi
            rc_skipped=$((rc_skipped + 1))
            continue
        fi

        # Capture AIMS ARC IR dump (exit code may be non-zero for test-only files)
        ORI_DUMP_AFTER_ARC=1 "$ORI_BIN" build "$test" -o /dev/null > /dev/null 2> "$AIMS_DATA/$key.arc" || true
        if [[ ! -s "$AIMS_DATA/$key.arc" ]]; then
            echo -e "  ${C_RED}RC SKIP (AIMS build failed): $rel_path${C_NC}"
            rc_skipped=$((rc_skipped + 1))
            continue
        fi

        old_rc=$(count_rc_ops "$OLD_DATA/$key.arc")
        aims_rc=$(count_rc_ops "$AIMS_DATA/$key.arc")
        total_old_rc=$((total_old_rc + old_rc))
        total_aims_rc=$((total_aims_rc + aims_rc))

        if [[ "$aims_rc" -lt "$old_rc" ]]; then
            if [[ "$VERBOSE" -eq 1 ]]; then
                echo -e "  ${C_GREEN}IMPROVEMENT ($old_rc -> $aims_rc, -$((old_rc - aims_rc))): $rel_path${C_NC}"
            fi
            RC_IMPROVEMENT_DETAILS+=("$rel_path: $old_rc -> $aims_rc (-$((old_rc - aims_rc)))")
            rc_improvements=$((rc_improvements + 1))
        elif [[ "$aims_rc" -gt "$old_rc" ]]; then
            echo -e "  ${C_YELLOW}REGRESSION ($old_rc -> $aims_rc, +$((aims_rc - old_rc))): $rel_path${C_NC}"
            RC_REGRESSION_DETAILS+=("$rel_path: $old_rc -> $aims_rc (+$((aims_rc - old_rc)))")
            rc_regressions=$((rc_regressions + 1))
        else
            rc_matches=$((rc_matches + 1))
        fi
    done <<< "$ALL_ORI_FILES"

    echo ""
    echo -e "${C_BOLD}RC Operations: ${C_GREEN}$rc_matches matches${C_NC}, " \
         "${C_GREEN}$rc_improvements improvements${C_NC}, " \
         "${C_YELLOW}$rc_regressions regressions${C_NC}, " \
         "${C_DIM}$rc_skipped skipped${C_NC}"
    echo -e "${C_BOLD}RC Totals: old=$total_old_rc, aims=$total_aims_rc" \
         "(delta: $((total_aims_rc - total_old_rc)))${C_NC}"

    if [[ ${#RC_IMPROVEMENT_DETAILS[@]} -gt 0 ]]; then
        echo ""
        echo -e "${C_GREEN}Top improvements:${C_NC}"
        printf '  %s\n' "${RC_IMPROVEMENT_DETAILS[@]}" | head -10
    fi

    if [[ ${#RC_REGRESSION_DETAILS[@]} -gt 0 ]]; then
        echo ""
        echo -e "${C_YELLOW}All regressions:${C_NC}"
        printf '  %s\n' "${RC_REGRESSION_DETAILS[@]}"
    fi

    echo ""
fi

# =============================================================================
# Summary
# =============================================================================

echo -e "${C_BOLD}=== Summary ===${C_NC}"

exit_code=0

if [[ "$RC_ONLY" -eq 0 ]]; then
    if [[ "$semantic_failures" -gt 0 ]]; then
        echo -e "${C_RED}FAILED: $semantic_failures behavioral regressions${C_NC}"
        exit_code=1
    else
        echo -e "${C_GREEN}Behavioral: PASSED ($semantic_matches programs verified)${C_NC}"
    fi
fi

if [[ "$BEHAVIORAL_ONLY" -eq 0 ]]; then
    if [[ "$rc_regressions" -gt 0 ]]; then
        echo -e "${C_YELLOW}RC: $rc_regressions regressions (investigate, not a hard failure)${C_NC}"
    else
        echo -e "${C_GREEN}RC: No regressions ($rc_improvements improvements, $rc_matches matches)${C_NC}"
    fi
fi

exit "$exit_code"
