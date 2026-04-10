#!/bin/bash
# Self-test for all diagnostic scripts.
#
# Usage:
#   diagnostics/self-test.sh [--verbose]
#
# Runs every diagnostic script on fixture programs, verifying expected
# output patterns (not exact match). Reports pass/fail per script per fixture.
#
# Fixtures: 20 entries (11 pass, 5 aims-heavy, 3 expected-fail, 1 infra).
# See diagnostics/fixtures/FIXTURES.md for the canonical fixture list and
# self-test contracts per category.
#
# Exit codes:
#   0 = all tests passed
#   1 = one or more tests failed
#   2 = usage error

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
FIXTURES_DIR="$SCRIPT_DIR/fixtures"
VERBOSE=0

# --- Parse arguments ---
while [[ $# -gt 0 ]]; do
    case $1 in
        --verbose) VERBOSE=1; shift ;;
        -h|--help)
            sed -n '2,/^$/{ s/^# \?//; p }' "$0"
            exit 0
            ;;
        *)
            echo "Error: unknown option: $1" >&2
            exit 2
            ;;
    esac
done

# --- Build compiler with LLVM support ---
echo "Building compiler (cargo b)..."
if ! (cd "$ROOT_DIR" && cargo b) 2>&1; then
    echo "Error: cargo build failed — cannot run diagnostic tests without LLVM-enabled binary" >&2
    exit 2
fi
echo ""

# --- Color codes ---
if [[ -t 1 ]]; then
    C_RED='\033[0;31m'
    C_GREEN='\033[0;32m'
    C_BOLD='\033[1m'
    C_DIM='\033[2m'
    C_NC='\033[0m'
else
    C_RED="" C_GREEN="" C_BOLD="" C_DIM="" C_NC=""
fi

# --- Counters ---
PASS=0
FAIL=0
SKIP=0

# --- Helper: run a test ---
# Usage: run_test "description" command [args...]
# Expects exit code 0. Captures stdout+stderr.
run_test() {
    local desc="$1"; shift
    local tmpout; tmpout=$(mktemp)

    if [[ "$VERBOSE" -eq 1 ]]; then
        printf "  ${C_DIM}Running: %s${C_NC}\n" "$*"
    fi

    if "$@" > "$tmpout" 2>&1; then
        printf "  ${C_GREEN}PASS${C_NC}  %s\n" "$desc"
        PASS=$((PASS + 1))
    else
        local rc=$?
        printf "  ${C_RED}FAIL${C_NC}  %s (exit %d)\n" "$desc" "$rc"
        if [[ "$VERBOSE" -eq 1 ]]; then
            sed 's/^/    /' "$tmpout"
        fi
        FAIL=$((FAIL + 1))
    fi
    rm -f "$tmpout"
}

# Usage: run_test_expect_fail "description" command [args...]
# Expects non-zero exit code.
run_test_expect_fail() {
    local desc="$1"; shift
    local tmpout; tmpout=$(mktemp)

    if "$@" > "$tmpout" 2>&1; then
        printf "  ${C_RED}FAIL${C_NC}  %s (expected failure, got success)\n" "$desc"
        FAIL=$((FAIL + 1))
    else
        printf "  ${C_GREEN}PASS${C_NC}  %s (correctly failed)\n" "$desc"
        PASS=$((PASS + 1))
    fi
    rm -f "$tmpout"
}

# Usage: run_test_exit_code "description" EXPECTED_CODE command [args...]
# Expects a specific exit code.
run_test_exit_code() {
    local desc="$1"
    local expected="$2"
    shift 2
    local tmpout; tmpout=$(mktemp)
    local rc=0

    "$@" > "$tmpout" 2>&1 || rc=$?

    if [[ $rc -eq $expected ]]; then
        printf "  ${C_GREEN}PASS${C_NC}  %s (exit %d as expected)\n" "$desc" "$rc"
        PASS=$((PASS + 1))
    else
        printf "  ${C_RED}FAIL${C_NC}  %s (expected exit %d, got %d)\n" "$desc" "$expected" "$rc"
        FAIL=$((FAIL + 1))
    fi
    rm -f "$tmpout"
}

# Usage: run_test_output_contains "description" "pattern" command [args...]
# Expects exit code 0 and stdout+stderr to contain pattern.
run_test_output_contains() {
    local desc="$1"
    local pattern="$2"
    shift 2
    local tmpout; tmpout=$(mktemp)

    if ! "$@" > "$tmpout" 2>&1; then
        local rc=$?
        # Some tools exit non-zero but produce valid output (e.g., rc-stats with imbalance)
        # Check if the output contains the pattern regardless
        :
    fi

    if grep -qF -- "$pattern" "$tmpout"; then
        printf "  ${C_GREEN}PASS${C_NC}  %s\n" "$desc"
        PASS=$((PASS + 1))
    else
        printf "  ${C_RED}FAIL${C_NC}  %s (output missing: '%s')\n" "$desc" "$pattern"
        if [[ "$VERBOSE" -eq 1 ]]; then
            echo "    --- output ---"
            sed 's/^/    /' "$tmpout"
            echo "    --- end ---"
        fi
        FAIL=$((FAIL + 1))
    fi
    rm -f "$tmpout"
}

# Usage: run_test_output_not_contains "description" "pattern" command [args...]
# Expects stdout+stderr to NOT contain pattern (any exit code allowed).
run_test_output_not_contains() {
    local desc="$1"
    local pattern="$2"
    shift 2
    local tmpout; tmpout=$(mktemp)

    "$@" > "$tmpout" 2>&1 || true

    if grep -qF -- "$pattern" "$tmpout"; then
        printf "  ${C_RED}FAIL${C_NC}  %s (output unexpectedly contains: '%s')\n" "$desc" "$pattern"
        FAIL=$((FAIL + 1))
    else
        printf "  ${C_GREEN}PASS${C_NC}  %s\n" "$desc"
        PASS=$((PASS + 1))
    fi
    rm -f "$tmpout"
}

# Usage: run_test_nonempty "description" command [args...]
# Expects exit code 0 and non-empty stdout.
run_test_nonempty() {
    local desc="$1"; shift
    local tmpout; tmpout=$(mktemp)

    if "$@" > "$tmpout" 2>&1; then
        if [[ -s "$tmpout" ]]; then
            printf "  ${C_GREEN}PASS${C_NC}  %s\n" "$desc"
            PASS=$((PASS + 1))
        else
            printf "  ${C_RED}FAIL${C_NC}  %s (empty output)\n" "$desc"
            FAIL=$((FAIL + 1))
        fi
    else
        local rc=$?
        printf "  ${C_RED}FAIL${C_NC}  %s (exit %d)\n" "$desc" "$rc"
        FAIL=$((FAIL + 1))
    fi
    rm -f "$tmpout"
}

# --- Check fixtures exist ---
# Canonical source: diagnostics/fixtures/FIXTURES.md
PASS_FIXTURES=(simple clean chain closure closure_escape iterator_break iterator_complex nested_list trait_dispatch pattern_match map_iteration)
AIMS_HEAVY_FIXTURES=(question_mark recursive_tree generic_mono large_aggregate cow_sharing)
EXPECTED_FAIL_FIXTURES=(leak)
ALL_FIXTURES=("${PASS_FIXTURES[@]}" "${AIMS_HEAVY_FIXTURES[@]}" "${EXPECTED_FAIL_FIXTURES[@]}")

for fixture in "${ALL_FIXTURES[@]}"; do
    if [[ ! -f "$FIXTURES_DIR/${fixture}.ori" ]]; then
        echo "Error: fixture not found: $FIXTURES_DIR/${fixture}.ori" >&2
        exit 2
    fi
done

printf "${C_BOLD}=== Diagnostic Toolkit Self-Test ===${C_NC}\n\n"

# ─── ir-dump.sh ────────────────────────────────────────────────────
printf "${C_BOLD}ir-dump.sh${C_NC}\n"
run_test_nonempty "simple.ori produces non-empty IR" \
    "$SCRIPT_DIR/ir-dump.sh" --no-color "$FIXTURES_DIR/simple.ori"
run_test_nonempty "clean.ori produces non-empty IR" \
    "$SCRIPT_DIR/ir-dump.sh" --no-color "$FIXTURES_DIR/clean.ori"
run_test_nonempty "chain.ori produces non-empty IR" \
    "$SCRIPT_DIR/ir-dump.sh" --no-color "$FIXTURES_DIR/chain.ori"
echo ""

# ─── ir-diff.sh ────────────────────────────────────────────────────
printf "${C_BOLD}ir-diff.sh${C_NC}\n"
# diff exits 1 when differences found (expected for different programs)
run_test_output_contains "simple vs chain shows differences" "define" \
    "$SCRIPT_DIR/ir-diff.sh" --no-color "$FIXTURES_DIR/simple.ori" "$FIXTURES_DIR/chain.ori"
echo ""

# ─── rc-stats.sh ──────────────────────────────────────────────────
printf "${C_BOLD}rc-stats.sh${C_NC}\n"
run_test "simple.ori has balanced RC (or no RC)" \
    "$SCRIPT_DIR/rc-stats.sh" --no-color "$FIXTURES_DIR/simple.ori"
# clean.ori shows imbalance because runtime functions (push_cow, reverse_cow)
# allocate internally — allocs are invisible in IR, only decs are visible.
run_test_output_contains "clean.ori produces RC stats" "Function" \
    "$SCRIPT_DIR/rc-stats.sh" --no-color "$FIXTURES_DIR/clean.ori"
run_test_output_contains "clean.ori --block-level shows block labels" "bb" \
    "$SCRIPT_DIR/rc-stats.sh" --no-color --block-level "$FIXTURES_DIR/clean.ori"
run_test_output_contains "clean.ori --block-level --optimized shows optimized blocks" "bb" \
    "$SCRIPT_DIR/rc-stats.sh" --no-color --block-level --optimized "$FIXTURES_DIR/clean.ori"
run_test_output_contains "clean.ori --optimized shows function stats" "Function" \
    "$SCRIPT_DIR/rc-stats.sh" --no-color --optimized "$FIXTURES_DIR/clean.ori"
echo ""

# ─── codegen-audit.sh ─────────────────────────────────────────────
printf "${C_BOLD}codegen-audit.sh${C_NC}\n"
run_test "simple.ori clean" \
    "$SCRIPT_DIR/codegen-audit.sh" --no-color "$FIXTURES_DIR/simple.ori"
run_test "clean.ori clean" \
    "$SCRIPT_DIR/codegen-audit.sh" --no-color "$FIXTURES_DIR/clean.ori"
run_test "chain.ori clean" \
    "$SCRIPT_DIR/codegen-audit.sh" --no-color "$FIXTURES_DIR/chain.ori"
run_test "strict mode on clean.ori" \
    "$SCRIPT_DIR/codegen-audit.sh" --strict --no-color "$FIXTURES_DIR/clean.ori"
run_test "function filter on chain.ori" \
    "$SCRIPT_DIR/codegen-audit.sh" --function main --no-color "$FIXTURES_DIR/chain.ori"
echo ""

# ─── diagnose-aot.sh ──────────────────────────────────────────────
printf "${C_BOLD}diagnose-aot.sh${C_NC}\n"
run_test "simple.ori passes all checks" \
    "$SCRIPT_DIR/diagnose-aot.sh" --no-color "$FIXTURES_DIR/simple.ori"
run_test "clean.ori passes all checks" \
    "$SCRIPT_DIR/diagnose-aot.sh" --no-color "$FIXTURES_DIR/clean.ori"
run_test_output_contains "diagnose-aot.sh --help shows --release" "--release" \
    "$SCRIPT_DIR/diagnose-aot.sh" --help
run_test_output_contains "diagnose-aot.sh --help shows --both-builds" "--both-builds" \
    "$SCRIPT_DIR/diagnose-aot.sh" --help
run_test_output_contains "diagnose-aot.sh --help shows Codegen Audit section" "Codegen Audit" \
    "$SCRIPT_DIR/diagnose-aot.sh" --help
run_test_output_contains "diagnose-aot.sh --help shows ARC IR section" "ARC IR" \
    "$SCRIPT_DIR/diagnose-aot.sh" --help
if [[ -x "$ROOT_DIR/target/release/ori" ]]; then
    run_test "diagnose-aot.sh --release on simple.ori" \
        "$SCRIPT_DIR/diagnose-aot.sh" --release --no-color "$FIXTURES_DIR/simple.ori"
    run_test_output_contains "diagnose-aot.sh --release shows (release) in header" "(release)" \
        "$SCRIPT_DIR/diagnose-aot.sh" --release --no-color "$FIXTURES_DIR/simple.ori"
    run_test "diagnose-aot.sh --both-builds on simple.ori" \
        "$SCRIPT_DIR/diagnose-aot.sh" --both-builds --no-color "$FIXTURES_DIR/simple.ori"
    run_test_output_contains "diagnose-aot.sh --both-builds shows per-section comparison" "COMPARISON" \
        "$SCRIPT_DIR/diagnose-aot.sh" --both-builds --no-color "$FIXTURES_DIR/simple.ori"
else
    printf "  ${C_DIM}SKIP${C_NC}  --release tests — release binary not found (run: cargo b --release)\n"
    SKIP=$((SKIP + 4))
fi
echo ""

# ─── dual-exec-debug.sh ───────────────────────────────────────────
printf "${C_BOLD}dual-exec-debug.sh${C_NC}\n"
run_test "simple.ori interpreter == AOT" \
    "$SCRIPT_DIR/dual-exec-debug.sh" --no-color "$FIXTURES_DIR/simple.ori"
run_test "clean.ori interpreter == AOT" \
    "$SCRIPT_DIR/dual-exec-debug.sh" --no-color "$FIXTURES_DIR/clean.ori"

# Mismatch path: verify auto-diagnostics output (uses ORI_BIN wrapper for deterministic divergence)
SAVED_ORI_BIN="${ORI_BIN:-}"
export ORI_BIN="$FIXTURES_DIR/mismatch-wrapper.sh"
run_test_exit_code "mismatch wrapper triggers mismatch (exit 1)" 1 \
    "$SCRIPT_DIR/dual-exec-debug.sh" --no-color "$FIXTURES_DIR/mismatch.ori"
run_test_output_contains "mismatch auto-dumps ARC IR" "ARC IR saved to" \
    "$SCRIPT_DIR/dual-exec-debug.sh" --no-color "$FIXTURES_DIR/mismatch.ori"
run_test_output_contains "mismatch runs codegen-audit" "Codegen Audit" \
    "$SCRIPT_DIR/dual-exec-debug.sh" --no-color "$FIXTURES_DIR/mismatch.ori"
run_test_output_contains "mismatch shows keep-temp hint" "keep-temp" \
    "$SCRIPT_DIR/dual-exec-debug.sh" --no-color "$FIXTURES_DIR/mismatch.ori"
if [[ -n "$SAVED_ORI_BIN" ]]; then
    export ORI_BIN="$SAVED_ORI_BIN"
else
    unset ORI_BIN
fi

# Build-failure path: verify exit code 2 and ARC IR capture attempt
run_test_exit_code "build failure exits 2 (not 1)" 2 \
    "$SCRIPT_DIR/dual-exec-debug.sh" --no-color "$FIXTURES_DIR/build-fail-parse.ori"
run_test_output_contains "build failure shows ARC IR status" "ARC IR" \
    "$SCRIPT_DIR/dual-exec-debug.sh" --no-color "$FIXTURES_DIR/build-fail-parse.ori"
echo ""

# ─── disasm-ori.sh ─────────────────────────────────────────────────
printf "${C_BOLD}disasm-ori.sh${C_NC}\n"
run_test_nonempty "simple.ori produces disassembly" \
    "$SCRIPT_DIR/disasm-ori.sh" --no-color "$FIXTURES_DIR/simple.ori"
echo ""

# ─── check-debug-flags.sh ─────────────────────────────────────────
printf "${C_BOLD}check-debug-flags.sh${C_NC}\n"
run_test "all debug flags consistent" \
    "$SCRIPT_DIR/check-debug-flags.sh"
echo ""

# ─── valgrind-aot.sh ──────────────────────────────────────────────
printf "${C_BOLD}valgrind-aot.sh${C_NC}\n"
run_test_output_contains "valgrind-aot.sh --help shows usage" "Usage:" \
    "$SCRIPT_DIR/valgrind-aot.sh" --help
# Skip actual Valgrind execution (may not be installed, slow)
echo ""

# ─── dual-exec-verify.sh ────────────────────────────────────────
printf "${C_BOLD}dual-exec-verify.sh${C_NC}\n"
run_test_output_contains "dual-exec-verify.sh --help shows usage" "Usage:" \
    "$SCRIPT_DIR/dual-exec-verify.sh" --help
run_test_expect_fail "dual-exec-verify.sh with nonexistent path" \
    "$SCRIPT_DIR/dual-exec-verify.sh" --no-color /nonexistent/path
# Skip actual batch execution (requires both binaries + full test suite)
echo ""

# ─── debug-release-compare.sh ─────────────────────────────────────
printf "${C_BOLD}debug-release-compare.sh${C_NC}\n"
if [[ -x "$ROOT_DIR/target/release/ori" ]]; then
    run_test "simple.ori debug == release" \
        "$SCRIPT_DIR/debug-release-compare.sh" --no-color "$FIXTURES_DIR/simple.ori"
    run_test "clean.ori debug == release" \
        "$SCRIPT_DIR/debug-release-compare.sh" --no-color "$FIXTURES_DIR/clean.ori"
else
    printf "  ${C_DIM}SKIP${C_NC}  release binary not found — run: cargo b --release\n"
    SKIP=$((SKIP + 2))
fi
run_test_output_contains "debug-release-compare.sh --help shows usage" "Usage:" \
    "$SCRIPT_DIR/debug-release-compare.sh" --help
run_test_expect_fail "debug-release-compare.sh with no args" \
    "$SCRIPT_DIR/debug-release-compare.sh" --no-color
# Test infrastructure error exit code (exit 2) on invalid input
bad_file=$(mktemp /tmp/self-test-bad-XXXX.ori)
printf '@main () -> void = { let x = }\n' > "$bad_file"
run_test_exit_code "debug-release-compare.sh exits 2 on compile failure" 2 \
    "$SCRIPT_DIR/debug-release-compare.sh" --no-color "$bad_file"
rm -f "$bad_file"
echo ""

# ─── bisect-passes.sh ─────────────────────────────────────────────
printf "${C_BOLD}bisect-passes.sh${C_NC}\n"
run_test_output_contains "bisect-passes.sh --help shows usage" "Usage:" \
    "$SCRIPT_DIR/bisect-passes.sh" --help
run_test_output_contains "bisect-passes.sh fixtures/simple.ori runs" "Phase" \
    "$SCRIPT_DIR/bisect-passes.sh" --no-color "$FIXTURES_DIR/simple.ori"
run_test_output_contains "bisect-passes.sh fixtures/clean.ori shows phases" "realize_rc_reuse" \
    "$SCRIPT_DIR/bisect-passes.sh" --no-color "$FIXTURES_DIR/clean.ori"
run_test_output_contains "bisect-passes.sh --function main filters" "Function: main" \
    "$SCRIPT_DIR/bisect-passes.sh" --no-color --function main "$FIXTURES_DIR/clean.ori"
run_test_expect_fail "bisect-passes.sh with no args" \
    "$SCRIPT_DIR/bisect-passes.sh" --no-color
run_test_expect_fail "bisect-passes.sh --function without value" \
    "$SCRIPT_DIR/bisect-passes.sh" --no-color --function
run_test_exit_code "bisect-passes.sh --rc-only simple.ori exits 0 (no RC divergence)" 0 \
    "$SCRIPT_DIR/bisect-passes.sh" --no-color --rc-only "$FIXTURES_DIR/simple.ori"
echo ""

# ─── Expanded Fixture Coverage ─────────────────────────────────────
# Canonical fixture list: diagnostics/fixtures/FIXTURES.md
printf "${C_BOLD}Pass fixtures (standard checks)${C_NC}\n"
for f in "${PASS_FIXTURES[@]}"; do
    # Skip the original 3 (already tested above in per-script sections)
    if [[ "$f" == "simple" || "$f" == "clean" || "$f" == "chain" ]]; then
        continue
    fi
    run_test_nonempty "${f}.ori ir-dump" \
        "$SCRIPT_DIR/ir-dump.sh" --no-color "$FIXTURES_DIR/${f}.ori"
    run_test_nonempty "${f}.ori arc-dump" \
        "$SCRIPT_DIR/arc-dump.sh" --no-color "$FIXTURES_DIR/${f}.ori"
    run_test "${f}.ori diagnose-aot" \
        "$SCRIPT_DIR/diagnose-aot.sh" --no-color "$FIXTURES_DIR/${f}.ori"
    run_test "${f}.ori dual-exec MATCH" \
        "$SCRIPT_DIR/dual-exec-debug.sh" --no-color "$FIXTURES_DIR/${f}.ori"
    run_test_output_contains "${f}.ori rc-stats produces output" "Function" \
        "$SCRIPT_DIR/rc-stats.sh" --no-color "$FIXTURES_DIR/${f}.ori"
    run_test_output_contains "${f}.ori bisect-passes produces phase table" "Phase" \
        "$SCRIPT_DIR/bisect-passes.sh" --no-color --rc-only "$FIXTURES_DIR/${f}.ori"
    run_test_output_contains "${f}.ori bisect-passes leak check clean" "Leak check: clean" \
        "$SCRIPT_DIR/bisect-passes.sh" --no-color --rc-only "$FIXTURES_DIR/${f}.ori"
done
# Feature-specific assertions for select pass fixtures
run_test_output_contains "closure.ori ARC has PartialApply" "PartialApply" \
    "$SCRIPT_DIR/arc-dump.sh" --no-color "$FIXTURES_DIR/closure.ori"
run_test_output_contains "closure_escape.ori ARC has PartialApply" "PartialApply" \
    "$SCRIPT_DIR/arc-dump.sh" --no-color "$FIXTURES_DIR/closure_escape.ori"
run_test_output_contains "pattern_match.ori ARC has Switch" "Switch" \
    "$SCRIPT_DIR/arc-dump.sh" --no-color "$FIXTURES_DIR/pattern_match.ori"
echo ""

printf "${C_BOLD}Aims-heavy fixtures (standard + feature-specific)${C_NC}\n"
for f in "${AIMS_HEAVY_FIXTURES[@]}"; do
    run_test_nonempty "${f}.ori ir-dump" \
        "$SCRIPT_DIR/ir-dump.sh" --no-color "$FIXTURES_DIR/${f}.ori"
    run_test_nonempty "${f}.ori arc-dump" \
        "$SCRIPT_DIR/arc-dump.sh" --no-color "$FIXTURES_DIR/${f}.ori"
    run_test "${f}.ori diagnose-aot" \
        "$SCRIPT_DIR/diagnose-aot.sh" --no-color "$FIXTURES_DIR/${f}.ori"
    run_test "${f}.ori dual-exec MATCH" \
        "$SCRIPT_DIR/dual-exec-debug.sh" --no-color "$FIXTURES_DIR/${f}.ori"
    run_test_output_contains "${f}.ori rc-stats produces output" "Function" \
        "$SCRIPT_DIR/rc-stats.sh" --no-color "$FIXTURES_DIR/${f}.ori"
    run_test_output_contains "${f}.ori bisect-passes produces phase table" "Phase" \
        "$SCRIPT_DIR/bisect-passes.sh" --no-color --rc-only "$FIXTURES_DIR/${f}.ori"
    run_test_output_contains "${f}.ori bisect-passes leak check clean" "Leak check: clean" \
        "$SCRIPT_DIR/bisect-passes.sh" --no-color --rc-only "$FIXTURES_DIR/${f}.ori"
done
# Feature-specific assertions
run_test_output_contains "generic_mono.ori ARC has multiple monomorphizations" "functions" \
    "$SCRIPT_DIR/arc-dump.sh" --no-color "$FIXTURES_DIR/generic_mono.ori"
run_test_output_contains "question_mark.ori ARC has RcDec (early exit cleanup)" "RcDec" \
    "$SCRIPT_DIR/arc-dump.sh" --no-color "$FIXTURES_DIR/question_mark.ori"
run_test_output_contains "recursive_tree.ori ARC has multiple functions" "functions" \
    "$SCRIPT_DIR/arc-dump.sh" --no-color "$FIXTURES_DIR/recursive_tree.ori"
run_test_output_contains "cow_sharing.ori ARC has RcInc (sharing)" "RcInc" \
    "$SCRIPT_DIR/arc-dump.sh" --no-color "$FIXTURES_DIR/cow_sharing.ori"
echo ""

printf "${C_BOLD}Expected-fail fixtures${C_NC}\n"
# leak.ori: diagnose-aot reports failure + imbalance
run_test_expect_fail "leak.ori diagnose-aot exits non-zero" \
    "$SCRIPT_DIR/diagnose-aot.sh" --no-color "$FIXTURES_DIR/leak.ori"
run_test_output_contains "leak.ori diagnose-aot shows imbalance" "imbalance" \
    "$SCRIPT_DIR/diagnose-aot.sh" --no-color "$FIXTURES_DIR/leak.ori"
# Panic bypasses the runtime leak checker (exits before ori_run_main checks
# RC_LIVE_COUNT), so bisect-passes reports "Leak check: clean" even for leak.ori.
# Instead, verify the runtime failure is detected.
run_test_output_contains "leak.ori bisect-passes detects runtime failure" "exited with code 1" \
    "$SCRIPT_DIR/bisect-passes.sh" --no-color --rc-only "$FIXTURES_DIR/leak.ori"
# mismatch.ori (via wrapper): dual-exec detects MISMATCH
SAVED_ORI_BIN2="${ORI_BIN:-}"
export ORI_BIN="$FIXTURES_DIR/mismatch-wrapper.sh"
run_test_exit_code "mismatch.ori dual-exec exits 1" 1 \
    "$SCRIPT_DIR/dual-exec-debug.sh" --no-color "$FIXTURES_DIR/mismatch.ori"
run_test_output_contains "mismatch.ori dual-exec shows MISMATCH" "MISMATCH" \
    "$SCRIPT_DIR/dual-exec-debug.sh" --no-color "$FIXTURES_DIR/mismatch.ori"
if [[ -n "$SAVED_ORI_BIN2" ]]; then
    export ORI_BIN="$SAVED_ORI_BIN2"
else
    unset ORI_BIN
fi
echo ""

# ─── Release build coverage ──────────────────────────────────────
printf "${C_BOLD}Release build coverage${C_NC}\n"
if [[ -x "$ROOT_DIR/target/release/ori" ]]; then
    for f in closure iterator_break generic_mono; do
        run_test "${f}.ori diagnose-aot --release" \
            "$SCRIPT_DIR/diagnose-aot.sh" --release --no-color "$FIXTURES_DIR/${f}.ori"
    done
else
    printf "  ${C_DIM}SKIP${C_NC}  release binary not found — run: cargo b --release\n"
    SKIP=$((SKIP + 3))
fi
echo ""

# ─── Error handling ───────────────────────────────────────────────
printf "${C_BOLD}Error handling${C_NC}\n"
run_test_expect_fail "codegen-audit.sh with no args" \
    "$SCRIPT_DIR/codegen-audit.sh" --no-color
run_test_expect_fail "codegen-audit.sh with nonexistent file" \
    "$SCRIPT_DIR/codegen-audit.sh" --no-color /nonexistent/file.ori
run_test_expect_fail "rc-stats.sh with no args" \
    "$SCRIPT_DIR/rc-stats.sh" --no-color
run_test_output_contains "codegen-audit.sh --help shows usage" "Usage:" \
    "$SCRIPT_DIR/codegen-audit.sh" --help
echo ""

# ─── Summary ──────────────────────────────────────────────────────
TOTAL=$((PASS + FAIL + SKIP))
printf "${C_BOLD}=== Summary ===${C_NC}\n"
printf "  ${C_GREEN}%d passed${C_NC}, " "$PASS"
if [[ "$FAIL" -gt 0 ]]; then
    printf "${C_RED}%d failed${C_NC}" "$FAIL"
else
    printf "%d failed" "$FAIL"
fi
if [[ "$SKIP" -gt 0 ]]; then
    printf ", %d skipped" "$SKIP"
fi
printf " (total: %d)\n" "$TOTAL"

if [[ "$FAIL" -gt 0 ]]; then
    exit 1
fi
exit 0
