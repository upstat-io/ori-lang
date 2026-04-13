#!/usr/bin/env bash
# Alive2 translation validation for Ori compiler.
#
# Verifies that LLVM optimization passes preserve the semantics of Ori's
# emitted IR by running alive-tv on pre-opt vs post-opt LLVM IR.
#
# Usage:
#   diagnostics/alive2-verify.sh [OPTIONS] <file.ori | --corpus>
#
# Options:
#   --corpus           Run against curated corpus (tests/alive2/curated-corpus.txt)
#   --all-codegen      Run against all .ori files in compiler/ori_llvm/tests/codegen/
#   --function NAME    Verify only the named function
#   --timeout SECS     Per-function Z3 timeout (default: 60)
#   --opt-level LEVEL  Ori optimization level (default: 2)
#   --verbose          Show alive-tv output for passing functions
#   --json             Machine-readable output to build/alive2-results/results.json
#   --suppress FILE    False positive suppression file (default: tests/alive2/suppressed.json)
#   --strict           Ignore all suppressions (for deep manual verification)
#   --check-survival   Verify that target functions survive optimization (not inlined away)
#   --no-color         Disable color output
#   -h, --help         Show this help message
#
# Exit codes:
#   0 = all functions verified or suppressed (no failures)
#   1 = one or more verification failures
#   2 = usage error or build failure

set -uo pipefail

# --- Defaults ---
CORPUS_MODE=false
ALL_CODEGEN_MODE=false
REVIEW_SUPPRESSIONS=false
TARGET_FUNCTION=""
Z3_TIMEOUT=60
OPT_LEVEL=2
VERBOSE=false
JSON_OUTPUT=false
SUPPRESS_FILE="tests/alive2/suppressed.json"
STRICT=false
CHECK_SURVIVAL=false
NO_COLOR=false
ORI_FILE=""

# --- Color helpers ---
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
RESET='\033[0m'

color_enabled() {
    [[ "$NO_COLOR" == "false" ]] && [[ -t 1 ]]
}

cecho() {
    local color="$1" msg="$2"
    if color_enabled; then
        printf "${color}%s${RESET}\n" "$msg"
    else
        printf "%s\n" "$msg"
    fi
}

# --- Parse arguments ---
usage() {
    sed -n '2,/^$/{ s/^# //; s/^#$//; p; }' "$0"
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --corpus) CORPUS_MODE=true; shift ;;
        --all-codegen) ALL_CODEGEN_MODE=true; shift ;;
        --review-suppressions) REVIEW_SUPPRESSIONS=true; shift ;;
        --function) TARGET_FUNCTION="$2"; shift 2 ;;
        --timeout) Z3_TIMEOUT="$2"; shift 2 ;;
        --opt-level) OPT_LEVEL="$2"; shift 2 ;;
        --verbose) VERBOSE=true; shift ;;
        --json) JSON_OUTPUT=true; shift ;;
        --suppress) SUPPRESS_FILE="$2"; shift 2 ;;
        --strict) STRICT=true; shift ;;
        --check-survival) CHECK_SURVIVAL=true; shift ;;
        --no-color) NO_COLOR=true; shift ;;
        -h|--help) usage; exit 0 ;;
        -*) echo "ERROR: Unknown option: $1" >&2; usage >&2; exit 2 ;;
        *) ORI_FILE="$1"; shift ;;
    esac
done

# --- Validate inputs ---
if [[ "$CORPUS_MODE" == "false" ]] && [[ "$ALL_CODEGEN_MODE" == "false" ]] && [[ "$REVIEW_SUPPRESSIONS" == "false" ]] && [[ -z "$ORI_FILE" ]]; then
    echo "ERROR: Provide a .ori file, --corpus, --all-codegen, or --review-suppressions" >&2
    exit 2
fi

# --- Find tools ---
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# Find alive-tv
ALIVE_TV="$ROOT_DIR/build/alive2/alive-tv"
if [[ ! -x "$ALIVE_TV" ]]; then
    echo "ERROR: alive-tv not found at $ALIVE_TV" >&2
    echo "  Run: ./scripts/build-alive2.sh" >&2
    exit 2
fi

# Find ori binary
ORI_BIN="${ORI_BIN:-$ROOT_DIR/target/debug/ori}"
if [[ ! -x "$ORI_BIN" ]]; then
    # Try release
    ORI_BIN="$ROOT_DIR/target/release/ori"
    if [[ ! -x "$ORI_BIN" ]]; then
        echo "ERROR: ori binary not found (tried target/debug/ori and target/release/ori)" >&2
        echo "  Run: cargo build" >&2
        exit 2
    fi
fi

RESULTS_DIR="$ROOT_DIR/build/alive2-results"
mkdir -p "$RESULTS_DIR"

# --- Counters ---
VERIFIED=0
FAILED=0
TIMEOUT_COUNT=0
SUPPRESSED=0
INLINED=0
ERRORS=0
TOTAL=0

# --- Extract function names from LLVM IR (without @ prefix) ---
extract_functions() {
    grep '^define' "$1" 2>/dev/null | sed 's/.*@\"\{0,1\}\([^" (]*\)\"\{0,1\}.*/\1/'
}

# --- Check if a function+file pair is in the suppression list ---
# Uses python3 for reliable JSON parsing — available on all CI runners.
is_suppressed() {
    local func="$1" file="$2"
    if [[ "$STRICT" == "true" ]] || [[ ! -f "$SUPPRESS_FILE" ]]; then
        return 1
    fi
    python3 -c "
import json, sys
with open('$SUPPRESS_FILE') as f:
    entries = json.load(f)
for e in entries:
    if e.get('function') == '$func' and ('$file' == '' or e.get('file', '') == '$file'):
        sys.exit(0)
sys.exit(1)
" 2>/dev/null
}

# --- Get suppression category for a function ---
get_suppression_category() {
    local func="$1" file="$2"
    python3 -c "
import json
with open('$SUPPRESS_FILE') as f:
    entries = json.load(f)
for e in entries:
    if e.get('function') == '$func' and ('$file' == '' or e.get('file', '') == '$file'):
        print(e.get('category', 'unknown'))
        break
" 2>/dev/null
}

# --- Review suppressions: check if suppressed entries still need suppression ---
review_suppressions() {
    if [[ ! -f "$SUPPRESS_FILE" ]]; then
        cecho "$GREEN" "No suppression file — nothing to review"
        return 0
    fi
    local count removable=0
    count=$(python3 -c "import json; print(len(json.load(open('$SUPPRESS_FILE'))))" 2>/dev/null)
    if [[ "$count" == "0" ]]; then
        cecho "$GREEN" "Suppression file is empty — nothing to review"
        return 0
    fi
    cecho "$CYAN" "Reviewing $count suppressions..."
    python3 -c "
import json
with open('$SUPPRESS_FILE') as f:
    entries = json.load(f)
for e in entries:
    print(e.get('file', '?') + ' ' + e.get('function', '?') + ' ' + e.get('category', '?'))
" 2>/dev/null | while read -r file func category; do
        echo -n "  $func ($file, $category): "
        if [[ ! -f "$ROOT_DIR/$file" ]]; then
            cecho "$YELLOW" "file missing — can remove"
            removable=$((removable + 1))
            continue
        fi
        # Try building and running alive-tv
        local capture_output
        capture_output=$(ORI_ALIVE2_CAPTURE=1 "$ORI_BIN" build "$ROOT_DIR/$file" --opt="$OPT_LEVEL" 2>&1)
        if [[ $? -ne 0 ]]; then
            echo "build failed — keep suppression"
            continue
        fi
        local basename
        basename=$(echo "$file" | sed 's|^\./||; s|/|_|g')
        basename="${basename%.ori}"
        local preopt="$RESULTS_DIR/${basename}.preopt.ll"
        local postopt="$RESULTS_DIR/${basename}.postopt.ll"
        if [[ ! -f "$preopt" ]] || [[ ! -f "$postopt" ]]; then
            echo "capture failed — keep suppression"
            continue
        fi
        local atv_output
        atv_output=$("$ALIVE_TV" "$preopt" "$postopt" --src-fn="$func" --tgt-fn="$func" --smt-to="$Z3_TIMEOUT" 2>&1)
        if echo "$atv_output" | grep -q "Transformation seems to be correct"; then
            cecho "$GREEN" "NOW PASSES — can remove suppression"
            removable=$((removable + 1))
        else
            echo "still fails — keep suppression"
        fi
    done
    cecho "$CYAN" "Review complete: $removable suppression(s) can be removed"
}

# --- Verify a single .ori file ---
verify_file() {
    local ori_file="$1"
    local file_verified=0
    local file_failed=0
    local file_inlined=0

    # Build with IR capture
    local capture_output
    capture_output=$(ORI_ALIVE2_CAPTURE=1 "$ORI_BIN" build "$ori_file" --opt="$OPT_LEVEL" 2>&1)
    local build_status=$?
    if [[ $build_status -ne 0 ]]; then
        cecho "$RED" "  BUILD FAILED: $ori_file"
        [[ "$VERBOSE" == "true" ]] && echo "$capture_output"
        ERRORS=$((ERRORS + 1))
        return 1
    fi

    # Find the captured IR files
    local basename
    basename=$(echo "$ori_file" | sed 's|^\./||; s|/|_|g')
    basename="${basename%.ori}"
    local preopt="$RESULTS_DIR/${basename}.preopt.ll"
    local postopt="$RESULTS_DIR/${basename}.postopt.ll"

    if [[ ! -f "$preopt" ]] || [[ ! -f "$postopt" ]]; then
        cecho "$RED" "  CAPTURE FAILED: IR files not generated for $ori_file"
        ERRORS=$((ERRORS + 1))
        return 1
    fi

    # Extract functions
    local preopt_funcs postopt_funcs
    preopt_funcs=$(extract_functions "$preopt")
    postopt_funcs=$(extract_functions "$postopt")

    # Determine target functions
    local target_funcs
    if [[ -n "$TARGET_FUNCTION" ]]; then
        target_funcs="$TARGET_FUNCTION"
    else
        # All _ori_ functions (skip runtime/personality/drop)
        target_funcs=$(echo "$preopt_funcs" | grep '^_ori_' | grep -v '^_ori_drop\$' | grep -v '^ori_eh_')
    fi

    for func in $target_funcs; do
        TOTAL=$((TOTAL + 1))

        # Check suppression (file+function pair)
        if is_suppressed "$func" "$ori_file"; then
            local cat
            cat=$(get_suppression_category "$func" "$ori_file")
            [[ "$VERBOSE" == "true" ]] && cecho "$YELLOW" "  SUPPRESSED: $func ($cat)"
            SUPPRESSED=$((SUPPRESSED + 1))
            continue
        fi

        # Check survival (inlining filter)
        if ! echo "$postopt_funcs" | grep -qF "$func"; then
            if [[ "$CHECK_SURVIVAL" == "true" ]]; then
                cecho "$YELLOW" "  INLINED: $func (absent from post-opt IR)"
            fi
            INLINED=$((INLINED + 1))
            file_inlined=$((file_inlined + 1))
            continue
        fi

        # Run alive-tv
        local atv_output
        atv_output=$("$ALIVE_TV" "$preopt" "$postopt" \
            --src-fn="$func" --tgt-fn="$func" --smt-to="$Z3_TIMEOUT" 2>&1)
        local atv_status=$?

        if echo "$atv_output" | grep -q "Transformation seems to be correct"; then
            [[ "$VERBOSE" == "true" ]] && cecho "$GREEN" "  VERIFIED: $func"
            VERIFIED=$((VERIFIED + 1))
            file_verified=$((file_verified + 1))
        elif echo "$atv_output" | grep -q "timeout"; then
            cecho "$YELLOW" "  TIMEOUT: $func (Z3 timeout at ${Z3_TIMEOUT}s)"
            TIMEOUT_COUNT=$((TIMEOUT_COUNT + 1))
        elif echo "$atv_output" | grep -q "ERROR"; then
            # alive-tv error (unsupported instruction, etc.)
            if is_suppressed "$func" "$ori_file"; then
                SUPPRESSED=$((SUPPRESSED + 1))
            else
                cecho "$YELLOW" "  UNSUPPORTED: $func"
                [[ "$VERBOSE" == "true" ]] && echo "$atv_output" | grep "ERROR:" | head -3
                ERRORS=$((ERRORS + 1))
            fi
        else
            cecho "$RED" "  FAILED: $func"
            [[ "$VERBOSE" == "true" ]] && echo "$atv_output"
            FAILED=$((FAILED + 1))
            file_failed=$((file_failed + 1))
        fi
    done

    return $file_failed
}

# --- Main ---
cecho "$CYAN" "Alive2 Translation Validation"
cecho "$CYAN" "alive-tv: $ALIVE_TV"
cecho "$CYAN" "ori: $ORI_BIN"
echo ""

EXIT_CODE=0

if [[ "$REVIEW_SUPPRESSIONS" == "true" ]]; then
    review_suppressions
    exit 0
fi

if [[ "$CORPUS_MODE" == "true" ]]; then
    CORPUS_FILE="$ROOT_DIR/tests/alive2/curated-corpus.txt"
    if [[ ! -f "$CORPUS_FILE" ]]; then
        echo "ERROR: Corpus file not found: $CORPUS_FILE" >&2
        exit 2
    fi
    cecho "$CYAN" "Running curated corpus: $CORPUS_FILE"
    echo ""

    while IFS=' ' read -r file func; do
        [[ -z "$file" || "$file" == "#"* ]] && continue
        if [[ -n "$func" ]]; then
            TARGET_FUNCTION="$func"
        else
            TARGET_FUNCTION=""
        fi
        cecho "$CYAN" "--- $file${func:+ ($func)} ---"
        if ! verify_file "$ROOT_DIR/$file"; then
            EXIT_CODE=1
        fi
    done < "$CORPUS_FILE"
elif [[ "$ALL_CODEGEN_MODE" == "true" ]]; then
    cecho "$CYAN" "Running against all codegen tests"
    echo ""
    while IFS= read -r -d '' file; do
        cecho "$CYAN" "--- $file ---"
        TARGET_FUNCTION=""
        if ! verify_file "$file"; then
            EXIT_CODE=1
        fi
    done < <(find "$ROOT_DIR/compiler/ori_llvm/tests/codegen" -name '*.ori' -print0 | sort -z)
else
    cecho "$CYAN" "--- $ORI_FILE ---"
    if ! verify_file "$ORI_FILE"; then
        EXIT_CODE=1
    fi
fi

# --- Summary ---
echo ""
cecho "$CYAN" "=== Alive2 Verification Summary ==="
echo "  Verified:   $VERIFIED"
echo "  Failed:     $FAILED"
echo "  Timeout:    $TIMEOUT_COUNT"
echo "  Suppressed: $SUPPRESSED"
echo "  Inlined:    $INLINED"
echo "  Errors:     $ERRORS"
echo "  Total:      $TOTAL"

if [[ $FAILED -gt 0 ]]; then
    cecho "$RED" "RESULT: $FAILED verification failure(s)"
    exit 1
fi

if [[ $ERRORS -gt 0 ]] && [[ $VERIFIED -eq 0 ]]; then
    cecho "$YELLOW" "RESULT: No verified functions (all errored)"
    exit 1
fi

cecho "$GREEN" "RESULT: All verifiable functions passed"
exit $EXIT_CODE
