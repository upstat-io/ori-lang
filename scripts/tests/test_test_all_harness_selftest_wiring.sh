#!/usr/bin/env bash
# Regression: every scripts/tests/*.sh file pins a specific test-all.sh
# correctness property (ANSI-summary parsing, build-race classification,
# prebuild-shape parity, JSON escaping, final exit-status semantics, AOT
# snapshot-integrity gate) but none of them were ever invoked by test-all.sh,
# CI, or lefthook - a regression in any pinned property would ship silently.
# Cure: test-all.sh runs every scripts/tests/*.sh as a blocking gate before
# any test leg starts (a harness-parsing/exit-status regression makes every
# downstream suite verdict untrustworthy, so it must fail loudly up front).
#
# Pins:
#  (a) WIRING - test-all.sh iterates scripts/tests/*.sh (this file's own
#      directory), not a hand-picked subset.
#  (b) BLOCKING - a self-test failure sets a fail flag that reaches `exit 1`
#      (no `|| true` / `2>/dev/null` swallowing the loop's verdict).
#  (c) ORDERING - the self-test gate runs BEFORE the "Build cache" section
#      (before any cargo build/test work begins), matching the debug-flag
#      and crate-DAG lint gates that already run pre-flight.
#  (d) LIVE - every scripts/tests/*.sh in the real directory currently
#      passes (this file itself included), so wiring the gate in does not
#      newly redden test-all.sh.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TESTALL="$HERE/../../test-all.sh"

if [ ! -f "$TESTALL" ]; then
    echo "FAIL: test-all.sh not found at $TESTALL"
    exit 1
fi

# (a) WIRING - test-all.sh loops over scripts/tests/*.sh
# shellcheck disable=SC2016 # literal pattern text, not a shell expansion
if ! grep -qE 'for[[:space:]]+_selftest[[:space:]]+in[[:space:]]+"\$TEST_ALL_DIR"/scripts/tests/\*\.sh' "$TESTALL"; then
    echo "FAIL: (a) test-all.sh does not iterate \"\$TEST_ALL_DIR\"/scripts/tests/*.sh (harness self-tests not wired)"
    exit 1
fi
echo "OK: (a) test-all.sh iterates scripts/tests/*.sh"

# (b) BLOCKING - a failing self-test reaches exit 1 via a fail-flag check,
# not a swallowed loop. Extract the self-test block (between the wiring
# marker and the "Build cache" comment) and assert it both records failure
# and checks it.
SELFTEST_BLOCK="$(awk '/Running test-all\.sh harness self-tests/{f=1} f{print} /^# Build cache:/{exit}' "$TESTALL")"
if [ -z "$SELFTEST_BLOCK" ]; then
    echo "FAIL: (b) could not extract the harness self-test block from test-all.sh"
    exit 1
fi
if ! printf '%s' "$SELFTEST_BLOCK" | grep -qE 'HARNESS_SELFTEST_FAILED=1'; then
    echo "FAIL: (b) self-test block never records a failure (no HARNESS_SELFTEST_FAILED=1 assignment)"
    exit 1
fi
# shellcheck disable=SC2016 # literal pattern text, not a shell expansion
if ! printf '%s' "$SELFTEST_BLOCK" | grep -qE '\$HARNESS_SELFTEST_FAILED.*-eq.*1'; then
    echo "FAIL: (b) self-test block never checks HARNESS_SELFTEST_FAILED before exiting (a recorded failure would be silently discarded)"
    exit 1
fi
if ! printf '%s' "$SELFTEST_BLOCK" | grep -qE '^\s*exit 1\s*$'; then
    echo "FAIL: (b) self-test block has no exit 1 on failure (not actually blocking)"
    exit 1
fi
echo "OK: (b) self-test block records + checks failure + exits 1 (genuinely blocking)"

# (c) ORDERING - the self-test gate precedes the Build cache section (i.e.
# precedes any cargo build/test invocation).
selftest_line="$(grep -n 'Running test-all\.sh harness self-tests' "$TESTALL" | head -1 | cut -d: -f1)"
buildcache_line="$(grep -n '^# Build cache:' "$TESTALL" | head -1 | cut -d: -f1)"
if [ -z "$selftest_line" ] || [ -z "$buildcache_line" ]; then
    echo "FAIL: (c) could not locate both the self-test gate and the Build cache section markers"
    exit 1
fi
if [ "$selftest_line" -ge "$buildcache_line" ]; then
    echo "FAIL: (c) harness self-test gate (line $selftest_line) does not precede the Build cache section (line $buildcache_line)"
    exit 1
fi
echo "OK: (c) harness self-test gate runs before any build/test work begins"

# (d) LIVE - every scripts/tests/*.sh currently passes, so the wiring does
# not itself redden test-all.sh. Run the sibling scripts (excluding this
# file, to avoid infinite self-recursion) directly.
live_failed=0
for _sibling in "$HERE"/*.sh; do
    [ -f "$_sibling" ] || continue
    case "$(basename "$_sibling")" in
        "$(basename "${BASH_SOURCE[0]}")") continue ;;
    esac
    if ! bash "$_sibling" > /dev/null 2>&1; then
        echo "FAIL: (d) $(basename "$_sibling") fails standalone (wiring it into a blocking gate would redden test-all.sh)"
        live_failed=1
    fi
done
if [ "$live_failed" -ne 0 ]; then
    exit 1
fi
echo "OK: (d) every sibling scripts/tests/*.sh passes standalone"

echo "PASS: test_test_all_harness_selftest_wiring - scripts/tests/*.sh is wired as a blocking pre-flight gate in test-all.sh"
