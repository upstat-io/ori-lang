#!/usr/bin/env bash
# Regression: an ANSI-colored nextest Summary line must parse to real counts.
# cargo-nextest honors FORCE_COLOR ahead of isatty, so a harness shell with
# FORCE_COLOR=3 makes nextest write SGR escape codes into the captured leg
# output even though it is redirected to a plain file. parse_rust_results'
# literal `grep -qE "Summary \["` cannot match across the ESC[0m between
# "Summary" and " [", so the parse-error branch mis-reports a ran leg as
# passed=0/failed=1 unattributed. Cure: strip ANSI (shared strip_ansi helper)
# before the grep chain; --color=never at the invocation is the source fix.
#
# Pins:
#  (a) POSITIVE — ANSI-colored CLEAN Summary parses real counts
#      (PASSED=4, FAILED=0), never the parse-error p0/f1 sentinel.
#  (b) NEGATIVE/SQUEEZE — ANSI-colored Summary WITH real failures still
#      reports them (FAILED=2); the cure must not mask genuine failures.
#  (c) REGRESSION — un-colored clean Summary still parses (no behavior
#      change on clean input).
#  (d) HELPER — the shared strip_ansi helper exists at scripts/lib/ansi.sh,
#      removes SGR sequences, and preserves all plain text byte-for-byte.
#  (e) PRECEDENCE — a BUILD_RACE_RE-matching output with a COLORED Summary
#      still classifies as race (FAILED=0 sentinel) BEFORE Summary parsing
#      (race-classification precedence preserved on colored input; see
#      test_test_all_nextest_race.sh for the full race pin suite).
#  (f) PRODUCTION PATH — the REAL rust_test_leg invocation site carries
#      --color=never on its cargo nextest run line, and running the REAL
#      rust_test_leg under FORCE_COLOR=3 yields output parse_rust_results
#      reads as real counts (PASSED>0, FAILED=0) — the leg function itself,
#      not an ad-hoc nextest call, is the surface under test.
#  (g) ENV LAYER — test-all.sh exports CARGO_TERM_COLOR=never and
#      NEXTEST_COLOR=never (belt-and-suspenders for the non-nextest cargo
#      invocations: doctests, plain cargo test fallback, builds).
#  (h) MIGRATION — diagnose-aot.sh no longer defines a private strip_ansi;
#      it sources the shared scripts/lib/ansi.sh helper (no third copy).
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LEG_HELPERS="$HERE/../test_all/legs.sh"
PARSING_HELPERS="$HERE/../test_all/parsing.sh"
ANSI_HELPER="$HERE/../lib/ansi.sh"

if [ ! -f "$LEG_HELPERS" ]; then
    echo "FAIL: leg helper file not found at $LEG_HELPERS"
    exit 1
fi
if [ ! -f "$PARSING_HELPERS" ]; then
    echo "FAIL: parsing helper file not found at $PARSING_HELPERS"
    exit 1
fi

# Extract globals + functions under test from the sourced helper files.
eval "$(sed -n '/^BUILD_RACE_RE=/p' "$LEG_HELPERS")"
if [ -z "${BUILD_RACE_RE:-}" ]; then
    echo "FAIL: BUILD_RACE_RE not defined in $LEG_HELPERS"
    exit 1
fi
# Source the shared ANSI helper when present (parse_rust_results depends on
# strip_ansi post-fix); pin (d) asserts its existence + behavior explicitly.
if [ -f "$ANSI_HELPER" ]; then
    # shellcheck source=/dev/null
    . "$ANSI_HELPER"
fi
eval "$(sed -n '/^parse_rust_results()/,/^}/p' "$PARSING_HELPERS")"

# parse_rust_results is shell-option-agnostic: its internal count pipelines
# carry `|| true` so an absent token (clean Summary has no "failed") never
# escapes to the caller. Invoking under THIS harness's `set -euo pipefail`
# (stricter than production test-all.sh, which has no pipefail) pins that
# robustness property.
run_parse() {
    NEXTEST_ACTIVE=1 parse_rust_results "$@"
}

ESC=$'\x1b'
# Faithful shape of a FORCE_COLOR-colored nextest Summary (SGR around tokens).
colored_summary_clean="${ESC}[32;1m     Summary${ESC}[0m [   0.020s] ${ESC}[1m4${ESC}[0m tests run: ${ESC}[1m4${ESC}[0m ${ESC}[32;1mpassed${ESC}[0m, ${ESC}[1m0${ESC}[0m ${ESC}[33;1mskipped${ESC}[0m"
colored_summary_fails="${ESC}[31;1m     Summary${ESC}[0m [  12.0s] ${ESC}[1m5${ESC}[0m tests run: ${ESC}[1m3${ESC}[0m ${ESC}[32;1mpassed${ESC}[0m, ${ESC}[1m2${ESC}[0m ${ESC}[31;1mfailed${ESC}[0m, ${ESC}[1m0${ESC}[0m ${ESC}[33;1mskipped${ESC}[0m"

# (a) POSITIVE — colored clean Summary parses real counts
fix_a="$(mktemp)"
{
    printf '    PASS [   0.020s] (1/4) ori_stack tests::grows\n'
    printf '%s\n' "$colored_summary_clean"
} > "$fix_a"
run_parse "$fix_a" "COLORED"
rm -f "$fix_a"
if [ "${COLORED_PASSED:-unset}" != "4" ] || [ "${COLORED_FAILED:-unset}" != "0" ]; then
    echo "FAIL: (a) colored clean Summary parsed PASSED=${COLORED_PASSED:-unset} FAILED=${COLORED_FAILED:-unset} (expected 4/0 — ANSI must not break Summary parsing)"
    exit 1
fi
echo "OK: (a) colored clean Summary parses PASSED=4 FAILED=0"

# (b) NEGATIVE/SQUEEZE — colored Summary with real failures stays reported
fix_b="$(mktemp)"
{
    printf '    PASS [   0.020s] (1/5) ori_llvm::aot collections_ext::test_a\n'
    printf '    FAIL [   0.020s] (2/5) ori_llvm::aot collections_ext::test_b\n'
    printf '%s\n' "$colored_summary_fails"
} > "$fix_b"
run_parse "$fix_b" "CFAIL"
rm -f "$fix_b"
if [ "${CFAIL_FAILED:-unset}" != "2" ]; then
    echo "FAIL: (b) colored Summary with real failures reported FAILED=${CFAIL_FAILED:-unset} (expected 2 — the ANSI cure must NOT mask genuine failures)"
    exit 1
fi
echo "OK: (b) colored Summary with real failures still reports FAILED=2"

# (c) REGRESSION — un-colored clean Summary still parses
fix_c="$(mktemp)"
cat > "$fix_c" <<'EOF'
    PASS [   0.020s] (1/3) ori_stack tests::grows
     Summary [   0.020s] 3 tests run: 3 passed, 0 skipped
EOF
run_parse "$fix_c" "PLAIN"
rm -f "$fix_c"
if [ "${PLAIN_PASSED:-unset}" != "3" ] || [ "${PLAIN_FAILED:-unset}" != "0" ]; then
    echo "FAIL: (c) un-colored clean Summary parsed PASSED=${PLAIN_PASSED:-unset} FAILED=${PLAIN_FAILED:-unset} (expected 3/0 — no behavior change on clean input)"
    exit 1
fi
echo "OK: (c) un-colored clean Summary still parses PASSED=3 FAILED=0"

# (d) HELPER — shared strip_ansi exists and is lossless on plain text
if [ ! -f "$ANSI_HELPER" ]; then
    echo "FAIL: (d) shared ANSI helper not found at $ANSI_HELPER (strip_ansi must live in scripts/lib/ansi.sh — never a per-file copy)"
    exit 1
fi
if ! command -v strip_ansi >/dev/null 2>&1 && ! declare -F strip_ansi >/dev/null 2>&1; then
    echo "FAIL: (d) strip_ansi not defined after sourcing $ANSI_HELPER"
    exit 1
fi
stripped="$(printf '%s\n' "$colored_summary_clean" | strip_ansi)"
if [ "$stripped" != "     Summary [   0.020s] 4 tests run: 4 passed, 0 skipped" ]; then
    echo "FAIL: (d) strip_ansi output mismatch: '$stripped'"
    exit 1
fi
plain_line='plain text with [brackets] and 4 passed tokens'
if [ "$(printf '%s\n' "$plain_line" | strip_ansi)" != "$plain_line" ]; then
    echo "FAIL: (d) strip_ansi altered plain text (must be lossless on SGR-free input)"
    exit 1
fi
echo "OK: (d) shared strip_ansi exists, strips SGR, lossless on plain text"

# (e) PRECEDENCE — colored Summary + race signature still classifies race first
RACE_LINE='error: [double-spawn] failed to exec "deps/aot-x": No such file or directory (os error 2)'
fix_e="$(mktemp)"
{
    printf '    PASS [   0.020s] (1/3) ori_llvm::aot collections_ext::test_a\n'
    printf '%s\n' "$RACE_LINE"
    printf '%s\n' "$colored_summary_fails"
} > "$fix_e"
run_parse "$fix_e" "RACEC"
rm -f "$fix_e"
if [ "${RACEC_FAILED:-unset}" != "0" ]; then
    echo "FAIL: (e) colored race-contaminated output reported FAILED=${RACEC_FAILED:-unset} (expected 0 — BUILD_RACE_RE precedence must survive the ANSI cure)"
    exit 1
fi
echo "OK: (e) colored race-contaminated output still classifies race-first (FAILED=0)"

# (f) PRODUCTION PATH — the real rust_test_leg carries --color=never and
# survives a FORCE_COLOR=3 environment end-to-end.
if ! grep -qE 'cargo nextest run[^|]*--color=never' "$LEG_HELPERS"; then
    echo "FAIL: (f) rust_test_leg's cargo nextest run invocation in $LEG_HELPERS does not carry --color=never (the source fix)"
    exit 1
fi
REPO_ROOT="$(cd "$HERE/../.." && pwd)"
eval "$(sed -n '/^cargo_race_retry()/,/^}/p' "$LEG_HELPERS")"
eval "$(sed -n '/^rust_test_leg()/,/^}/p' "$LEG_HELPERS")"
out_f="$(mktemp)"
if ( cd "$REPO_ROOT" && NEXTEST_ACTIVE=1 FORCE_COLOR=3 rust_test_leg "$out_f" -p ori_stack --lib ); then
    run_parse "$out_f" "PROD"
    if [ "${PROD_PASSED:-0}" -lt 1 ] || [ "${PROD_FAILED:-unset}" != "0" ]; then
        echo "FAIL: (f) real rust_test_leg under FORCE_COLOR=3 parsed PASSED=${PROD_PASSED:-unset} FAILED=${PROD_FAILED:-unset} (expected PASSED>=1 FAILED=0)"
        rm -f "$out_f"
        exit 1
    fi
    echo "OK: (f) real rust_test_leg under FORCE_COLOR=3 parses real counts (PASSED=${PROD_PASSED})"
else
    echo "FAIL: (f) real rust_test_leg returned non-zero under FORCE_COLOR=3 (leg output at $out_f)"
    rm -f "$out_f"
    exit 1
fi
rm -f "$out_f"

# (g) ENV LAYER — test-all.sh forces color off for every cargo invocation
TESTALL="$HERE/../../test-all.sh"
if ! grep -qE '^export CARGO_TERM_COLOR=never' "$TESTALL" || ! grep -qE '^export NEXTEST_COLOR=never' "$TESTALL"; then
    echo "FAIL: (g) test-all.sh does not export CARGO_TERM_COLOR=never + NEXTEST_COLOR=never (env belt-and-suspenders for doctests / plain cargo test / builds)"
    exit 1
fi
echo "OK: (g) test-all.sh exports CARGO_TERM_COLOR=never + NEXTEST_COLOR=never"

# (h) MIGRATION — diagnose-aot.sh uses the shared helper, no private copy
DIAGNOSE="$HERE/../../diagnostics/diagnose-aot.sh"
if grep -qE '^\s*strip_ansi\(\)' "$DIAGNOSE"; then
    echo "FAIL: (h) diagnose-aot.sh still defines a private strip_ansi (must source scripts/lib/ansi.sh — no third copy)"
    exit 1
fi
if ! grep -qE '^[[:space:]]*(source|\.)[[:space:]]+[^#]*lib/ansi\.sh' "$DIAGNOSE"; then
    echo "FAIL: (h) diagnose-aot.sh does not source the shared scripts/lib/ansi.sh helper (an actual source/. command, not a mention)"
    exit 1
fi
echo "OK: (h) diagnose-aot.sh sources the shared ansi helper (no private strip_ansi)"

# (i) PRODUCTION WIRING — the production parse path actually sources the shared
# helper (the test-seam source at the top of this file must never be the only
# wiring; parsing.sh or test-all.sh must load strip_ansi for the real run).
if ! grep -qE '^[[:space:]]*(source|\.)[[:space:]]+[^#]*lib/ansi\.sh' "$PARSING_HELPERS" \
    && ! grep -qE '^[[:space:]]*(source|\.)[[:space:]]+[^#]*lib/ansi\.sh' "$TESTALL"; then
    echo "FAIL: (i) neither scripts/test_all/parsing.sh nor test-all.sh sources scripts/lib/ansi.sh — production parse path has no strip_ansi wiring"
    exit 1
fi
echo "OK: (i) production parse path sources the shared ansi helper"

echo "PASS: test_test_all_ansi_summary — ANSI-colored Summary parses real counts; failures unmasked; race precedence intact; production path + env layer + helper migration + production wiring pinned"
