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
#   --review-suppressions  Check if suppressed entries still need suppression (stale detection)
#   --classify-function PREOPT_LL FUNC  Print the Alive2-modellability of FUNC's pre-opt
#                      IR ("modellable" / "unmodellable:<class>"); needs no alive-tv/build
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
CLASSIFY_MODE=false
CLASSIFY_LL=""
CLASSIFY_FUNC=""

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

# --- Classify a function's Alive2-modellability from its pre-opt IR ---
# Scans the named function's body for constructs Alive2 cannot model, per
# tests/alive2/README.md "Corpus Selection Criteria" (the SSOT for the exclusion
# classes). Prints "modellable" or "unmodellable:<class>" (eh|variadic|cow|rc).
# Inputs are passed by ARGV (never interpolated into a shell-quoted heredoc) so
# $-bearing names (@"_ori_elem_dec$3") and injection are non-issues. The runtime
# prefix @ori_ (no leading underscore) distinguishes a runtime call from a
# user/compiled function (@_ori_*), so a @_ori_helper call stays modellable; the
# memory-family token match is deliberately broad (it catches embedded tokens like
# ori_list_alloc_data / ori_buffer_drop_unique), and @ori_ is reserved runtime.
classify_function_modellability() {
    local llfile="$1" func="$2"
    if [[ ! -f "$llfile" ]]; then
        echo "error: IR file not found: $llfile"
        return 2
    fi
    # Extract the function body, stripping LLVM `;` end-of-line comments so the
    # construct scans never match a word inside a comment (e.g. "; resume later").
    local body
    body=$(awk -v fn="$func" '
        BEGIN { infn = 0 }
        !infn && /^define/ {
            if (index($0, "@\"" fn "\"(") || index($0, "@" fn "(")) { infn = 1 }
        }
        infn { print }
        infn && /^}/ { infn = 0 }
    ' "$llfile" | sed 's/;.*//')
    if [[ -z "$body" ]]; then
        echo "error: function '$func' not found in $llfile"
        return 2
    fi
    # Exception handling (IR opcodes) — README Exclude: "Exception handling".
    if grep -Eq '(^|[[:space:]])(landingpad|resume)([[:space:]]|$)|(^|[[:space:]])invoke[[:space:]]' <<<"$body"; then
        echo "unmodellable:eh"; return 0
    fi
    # Variadic — README Exclude: "Variadic functions".
    if grep -Eq '(^|[[:space:]])va_arg([[:space:]]|$)|@llvm\.va_(start|end)|\.\.\.\)' <<<"$body"; then
        echo "unmodellable:variadic"; return 0
    fi
    # COW uniqueness — checked BEFORE rc. Covers ori_rc_is_unique +
    # ori_rc_is_unique_or_null (no trailing-( anchor so the _or_null variant
    # matches). @ori_<name> is a runtime symbol (user functions are @_ori_*).
    if grep -Eq '@ori_[a-zA-Z0-9_]*is_unique' <<<"$body"; then
        echo "unmodellable:cow"; return 0
    fi
    # Memory-management runtime calls — README Exclude "RC operations (Alive2 can't
    # model custom allocators)". The unmodellable memory family: custom allocators
    # (ori_*alloc*: ori_alloc, ori_rc_alloc, ori_list_alloc_data, ori_*_literal_alloc),
    # free (ori_rc_free, ori_list_free*), drop (ori_*drop*), refcount
    # (ori_*rc_(inc|dec) across every container), buffer ops (ori_*buffer*),
    # elem-dec drop-glue (ori_*elem_dec), and panic. PURE runtime calls
    # (ori_compare_*, ori_format_*, ori_iter_map/...) are NOT excluded — alive-tv
    # models them as consistent uninterpreted functions. @ori_ = runtime (user
    # functions are @_ori_*, so a @_ori_helper call stays modellable).
    if grep -Eq '@ori_[a-zA-Z0-9_]*(alloc|free|drop|rc_inc|rc_dec|buffer|elem_dec|panic)' <<<"$body"; then
        echo "unmodellable:rc"; return 0
    fi
    echo "modellable"
    return 0
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
        --classify-function) CLASSIFY_MODE=true; CLASSIFY_LL="${2:-}"; CLASSIFY_FUNC="${3:-}"; shift; [[ $# -gt 0 ]] && shift; [[ $# -gt 0 ]] && shift ;;
        --no-color) NO_COLOR=true; shift ;;
        -h|--help) usage; exit 0 ;;
        -*) echo "ERROR: Unknown option: $1" >&2; usage >&2; exit 2 ;;
        *) ORI_FILE="$1"; shift ;;
    esac
done

# --- Classify-function mode: pure pre-opt-IR modellability scan ---
# Short-circuits BEFORE the input/alive-tv/ORI_BIN guards — it needs neither a
# .ori target, the alive-tv binary, nor the ori compiler (only the .ll file).
if [[ "$CLASSIFY_MODE" == "true" ]]; then
    if [[ -z "$CLASSIFY_LL" ]] || [[ -z "$CLASSIFY_FUNC" ]]; then
        echo "ERROR: --classify-function requires <preopt.ll> <function>" >&2
        exit 2
    fi
    classify_function_modellability "$CLASSIFY_LL" "$CLASSIFY_FUNC"
    exit $?
fi

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

# JSON result accumulator (newline-separated JSON objects)
JSON_ENTRIES_FILE=""
if [[ "$JSON_OUTPUT" == "true" ]]; then
    JSON_ENTRIES_FILE="$(mktemp)"
fi

# --- Record a JSON result entry ---
json_record() {
    [[ "$JSON_OUTPUT" != "true" ]] && return
    local file="$1" func="$2" status="$3" category="${4:-}" output="${5:-}"
    # Escape strings for JSON
    output=$(echo "$output" | python3 -c "import sys,json; print(json.dumps(sys.stdin.read().strip()))" 2>/dev/null || echo '""')
    echo "{\"file\":\"$file\",\"function\":\"$func\",\"status\":\"$status\"${category:+,\"suppression_category\":\"$category\"},\"alive2_output\":$output}" >> "$JSON_ENTRIES_FILE"
}

# --- Extract function names from LLVM IR (without @ prefix) ---
extract_functions() {
    grep '^define' "$1" 2>/dev/null | sed 's/.*@\"\{0,1\}\([^" (]*\)\"\{0,1\}.*/\1/'
}

# --- Captured-IR paths for a .ori source (SSOT for the ir_capture.rs naming) ---
# ir_capture.rs names each captured IR file by the source path with a leading
# `./` stripped, every `/` -> `_`, and the `.ori` suffix dropped, under
# RESULTS_DIR. Echoes "<preopt.ll> <postopt.ll>"; the caller splits with `read`.
# Sanitizes whatever path it is handed (repo-relative in --review-suppressions,
# absolute in --all-codegen / single-file), so both modes share one mapping.
captured_ir_paths() {
    local stem
    stem=$(echo "$1" | sed 's|^\./||; s|/|_|g')
    stem="${stem%.ori}"
    echo "$RESULTS_DIR/${stem}.preopt.ll $RESULTS_DIR/${stem}.postopt.ll"
}

# --- Check if a function+file pair is in the suppression list ---
# Suppressed iff the SOLE match predicate (get_suppression_category) finds an
# entry — a non-empty category print means a file+function match.
is_suppressed() {
    local func="$1" file="$2"
    if [[ "$STRICT" == "true" ]] || [[ ! -f "$SUPPRESS_FILE" ]]; then
        return 1
    fi
    [[ -n "$(get_suppression_category "$func" "$file")" ]]
}

# --- Suppression-match predicate (SSOT for is_suppressed + category lookup) ---
# Prints the matching entry's category; the file+function match IS the
# suppression test, so empty output = not suppressed. suppressed.json stores
# REPO-RELATIVE file paths; the $file passed in --all-codegen / --corpus mode is
# ABSOLUTE, so normalize before comparing (else a file-pinned suppression
# silently never matches). Values are passed by ARGV (sys.argv), never
# interpolated into the python source (injection + $-bearing-name quoting
# hazard). Uses python3 for reliable JSON parsing — available on all CI runners.
get_suppression_category() {
    local func="$1" file="$2"
    local file_rel="${file#"$ROOT_DIR"/}"
    python3 - "$SUPPRESS_FILE" "$func" "$file_rel" "$file" <<'PY' 2>/dev/null
import json, sys
suppress_file, func, file_rel, file_abs = sys.argv[1:5]
with open(suppress_file) as f:
    entries = json.load(f)
for e in entries:
    if e.get('function') == func and (file_rel == '' or e.get('file', '') in (file_rel, file_abs)):
        print(e.get('category', 'unknown'))
        break
PY
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
    # Use process substitution to avoid pipe subshell (counters are lost in subshells)
    while read -r file func category; do
        echo -n "  $func ($file, $category): "
        if [[ ! -f "$ROOT_DIR/$file" ]]; then
            cecho "$YELLOW" "file missing — can remove"
            removable=$((removable + 1))
            continue
        fi
        # Build from ROOT_DIR with relative path so ir_capture.rs produces
        # consistent repo-relative filenames (not absolute-path-based names)
        local capture_output
        capture_output=$(cd "$ROOT_DIR" && ORI_ALIVE2_CAPTURE=1 "$ORI_BIN" build "$file" --opt="$OPT_LEVEL" 2>&1)
        if [[ $? -ne 0 ]]; then
            echo "build failed — keep suppression"
            continue
        fi
        local preopt postopt
        read -r preopt postopt < <(captured_ir_paths "$file")
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
    done < <(python3 -c "
import json
with open('$SUPPRESS_FILE') as f:
    entries = json.load(f)
for e in entries:
    print(e.get('file', '?') + ' ' + e.get('function', '?') + ' ' + e.get('category', '?'))
" 2>/dev/null)
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
    local preopt postopt
    read -r preopt postopt < <(captured_ir_paths "$ori_file")

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
            json_record "$ori_file" "$func" "suppressed" "$cat"
            continue
        fi

        # Check function exists in pre-opt IR (catches typos/renames in corpus)
        if ! echo "$preopt_funcs" | grep -qF "$func"; then
            cecho "$RED" "  MISSING: $func (not found in pre-opt IR — check function name)"
            FAILED=$((FAILED + 1))
            file_failed=$((file_failed + 1))
            json_record "$ori_file" "$func" "error" "" "Function not found in pre-opt IR"
            continue
        fi

        # Content-scan: skip functions Alive2 cannot model (RC / exception-handling
        # / COW / variadic per tests/alive2/README.md "Corpus Selection Criteria").
        # alive-tv reports a documented false positive on these (it can't model
        # custom allocators / invoke-landingpad / external side-effecting calls),
        # so they are recorded `suppressed`/`unmodellable`, NOT `failed`.
        local modellability
        modellability=$(classify_function_modellability "$preopt" "$func")
        if [[ "$modellability" == unmodellable:* ]]; then
            local uclass="${modellability#unmodellable:}"
            [[ "$VERBOSE" == "true" ]] && cecho "$YELLOW" "  UNMODELLABLE: $func ($uclass — Alive2 false-positive class)"
            SUPPRESSED=$((SUPPRESSED + 1))
            json_record "$ori_file" "$func" "suppressed" "unmodellable:$uclass"
            continue
        fi

        # Check survival (inlining filter) — present pre-opt but absent post-opt
        if ! echo "$postopt_funcs" | grep -qF "$func"; then
            if [[ "$CHECK_SURVIVAL" == "true" ]]; then
                cecho "$RED" "  INLINED: $func (present pre-opt, absent post-opt — survival violation)"
                FAILED=$((FAILED + 1))
                file_failed=$((file_failed + 1))
                json_record "$ori_file" "$func" "inlined"
            else
                [[ "$VERBOSE" == "true" ]] && cecho "$YELLOW" "  INLINED: $func (absent from post-opt IR)"
                INLINED=$((INLINED + 1))
                file_inlined=$((file_inlined + 1))
                json_record "$ori_file" "$func" "inlined"
            fi
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
            json_record "$ori_file" "$func" "verified"
        elif echo "$atv_output" | grep -q "timeout"; then
            cecho "$YELLOW" "  TIMEOUT: $func (Z3 timeout at ${Z3_TIMEOUT}s)"
            TIMEOUT_COUNT=$((TIMEOUT_COUNT + 1))
            json_record "$ori_file" "$func" "timeout" "" "$atv_output"
        elif echo "$atv_output" | grep -q "ERROR"; then
            # alive-tv error (unsupported instruction, etc.)
            if is_suppressed "$func" "$ori_file"; then
                SUPPRESSED=$((SUPPRESSED + 1))
                json_record "$ori_file" "$func" "suppressed"
            else
                cecho "$YELLOW" "  UNSUPPORTED: $func"
                [[ "$VERBOSE" == "true" ]] && echo "$atv_output" | grep "ERROR:" | head -3
                ERRORS=$((ERRORS + 1))
                json_record "$ori_file" "$func" "error" "" "$atv_output"
            fi
        else
            cecho "$RED" "  FAILED: $func"
            [[ "$VERBOSE" == "true" ]] && echo "$atv_output"
            FAILED=$((FAILED + 1))
            file_failed=$((file_failed + 1))
            json_record "$ori_file" "$func" "failed" "" "$atv_output"
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

# --- Write JSON results ---
if [[ "$JSON_OUTPUT" == "true" ]] && [[ -n "$JSON_ENTRIES_FILE" ]]; then
    JSON_FILE="$RESULTS_DIR/results.json"
    # Determine mode
    json_mode="single-file"
    [[ "$CORPUS_MODE" == "true" ]] && json_mode="curated"
    [[ "$ALL_CODEGEN_MODE" == "true" ]] && json_mode="full-sweep"
    # Extract alive2 commit from build script
    alive2_commit=$(grep 'ALIVE2_COMMIT=' "$ROOT_DIR/scripts/build-alive2.sh" 2>/dev/null | head -1 | cut -d'"' -f2)
    # alive-tv --version prints "LLVM version 21.0.0" (digits do NOT immediately
    # follow "LLVM " — they follow "version "), so match the optional "version"
    # word; handles both "LLVM version 21" and a bare "LLVM 21".
    llvm_major=$("$ROOT_DIR/build/alive2/alive-tv" --version 2>&1 \
        | grep -oiE 'LLVM[ -]+(version[ ]+)?[0-9]+' | grep -oE '[0-9]+' | head -1)
    if [[ -z "$llvm_major" ]]; then
        echo "WARNING: Could not parse LLVM version from alive-tv --version" >&2
        llvm_major="0"
    fi
    # Build JSON
    python3 -c "
import json, sys
from datetime import datetime, timezone
entries = []
with open('$JSON_ENTRIES_FILE') as f:
    for line in f:
        line = line.strip()
        if line:
            entries.append(json.loads(line))
result = {
    'version': 1,
    'timestamp': datetime.now(timezone.utc).isoformat(),
    'llvm_version': int('${llvm_major}'),
    'alive2_commit': '${alive2_commit}',
    'mode': '${json_mode}',
    'summary': {
        'verified': $VERIFIED,
        'failed': $FAILED,
        'timeout': $TIMEOUT_COUNT,
        'suppressed': $SUPPRESSED,
        'inlined': $INLINED,
        'errors': $ERRORS,
        'total': $TOTAL
    },
    'functions': entries
}
with open('$JSON_FILE', 'w') as f:
    json.dump(result, f, indent=2)
print(f'JSON results written to $JSON_FILE', file=sys.stderr)
" 2>&1 >&2
    rm -f "$JSON_ENTRIES_FILE"
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
