#!/usr/bin/env bash
# Regression: a concurrent-build race ("No such file or directory (os error 2)"
# / "[double-spawn] failed to exec") on a nextest leg must NOT serialize as a
# real `failed` count. The cargo-test path retries the race via cargo_race_retry;
# the nextest branch of rust_test_leg must route through the same generalized
# wrapper, and parse_rust_results must force the errored sentinel (FAILED=0 +
# nonzero leg exit -> suite_status=errored) on a race-contaminated Summary rather
# than parse its phantom `failed` count. Persistent race -> errored (non-verdict),
# never a phantom count; a clean run's genuine failures stay reported.
#
# Pins:
#  (a) BEHAVIORAL — generalized cargo_race_retry runs an arbitrary runner via
#      "$@" and retries once on the race signature: a fake runner that emits the
#      race on attempt 1 then succeeds on attempt 2 makes the wrapper retry to
#      success (2 attempts). Pre-cure cargo_race_retry hard-runs `cargo "$@"`,
#      so a non-cargo runner never retries -> this cell fails pre-cure.
#  (b) BEHAVIORAL — a runner that emits the race on BOTH attempts makes the
#      wrapper return non-zero (persistent race -> the leg's nonzero exit).
#  (c) POSITIVE PIN — parse_rust_results on a race-signature nextest output (with
#      a "Summary [...] N failed" line) yields FAILED=0, NOT the phantom N.
#  (d) NEGATIVE PIN — parse_rust_results on a CLEAN nextest output with real
#      failures STILL reports them (no masking / no over-suppression).
#  (e) SQUEEZE PIN — a Summary line carrying BOTH a real FAIL line AND a race
#      exec-failure yields FAILED=0 (whole-leg errored; the aggregate Summary
#      cannot separate real from exec failures, so the safe call is non-verdict).
#  (f) suite_status integration — (nonzero exit, FAILED=0) => errored non-verdict;
#      (nonzero exit, FAILED=2) => failed verdict (real failures stay a verdict).
#  (g) GUARD — an unset/empty BUILD_RACE_RE must never fall through to
#      `grep -qE ""`, which matches every non-empty line and would
#      misclassify every clean nextest Summary as a build-artifact race;
#      parse_rust_results must require BUILD_RACE_RE non-empty before the
#      race-signature grep.
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
if [ ! -f "$ANSI_HELPER" ]; then
    echo "FAIL: shared ANSI helper not found at $ANSI_HELPER"
    exit 1
fi

# Source the real production helpers under test directly (no sed-extraction:
# both files define only functions/globals with no top-level side effects, so
# sourcing is faithful to production and immune to a function-body-shape
# change silently truncating a text-extraction pattern). parsing.sh sources
# the shared ansi.sh helper itself, defining strip_ansi as a side effect
# (parse_rust_results depends on it post-fix).
# shellcheck source=/dev/null
. "$LEG_HELPERS"
if [ -z "${BUILD_RACE_RE:-}" ]; then
    echo "FAIL: BUILD_RACE_RE not defined in $LEG_HELPERS"
    exit 1
fi
# shellcheck source=/dev/null
. "$PARSING_HELPERS"

RACE_LINE='error: [double-spawn] failed to exec "deps/aot-x": No such file or directory (os error 2)'

# (a) BEHAVIORAL — generalized wrapper retries an arbitrary runner
attempt_a="$(mktemp)"; printf '0' > "$attempt_a"
race_then_pass() {
    local n; n=$(( $(cat "$attempt_a") + 1 )); printf '%s' "$n" > "$attempt_a"
    if [ "$n" -eq 1 ]; then printf '%s\n' "$RACE_LINE"; return 1; fi
    printf 'Summary [  1.0s] 3 tests run: 3 passed, 0 skipped\n'; return 0
}
out_a="$(mktemp)"
if cargo_race_retry "$out_a" race_then_pass; then a_rc=0; else a_rc=$?; fi
a_attempts="$(cat "$attempt_a")"
rm -f "$attempt_a" "$out_a"
if [ "$a_rc" != "0" ] || [ "$a_attempts" != "2" ]; then
    echo "FAIL: (a) generalized cargo_race_retry did not retry an arbitrary runner to success (rc=$a_rc attempts=$a_attempts; expected rc=0 attempts=2)"
    exit 1
fi
echo "OK: (a) generalized wrapper retries an arbitrary runner once on the race signature (2 attempts -> success)"

# (b) BEHAVIORAL — persistent race => non-zero return
race_always() { printf '%s\n' "$RACE_LINE"; return 1; }
out_b="$(mktemp)"
if cargo_race_retry "$out_b" race_always; then b_rc=0; else b_rc=$?; fi
rm -f "$out_b"
if [ "$b_rc" = "0" ]; then
    echo "FAIL: (b) persistent race returned rc=0 (expected non-zero so the leg exit drives suite_status=errored)"
    exit 1
fi
echo "OK: (b) persistent race returns non-zero (leg exit will drive errored)"

# (c) POSITIVE PIN — race signature => FAILED=0 (no phantom count)
race_fix="$(mktemp)"
cat > "$race_fix" <<EOF
    PASS [   0.020s] (1/3) ori_llvm::aot collections_ext::test_a
    $RACE_LINE
    Summary [  17.5s] 3 tests run: 1 passed, 2 failed, 0 skipped
EOF
NEXTEST_ACTIVE=1 parse_rust_results "$race_fix" "RACE"
rm -f "$race_fix"
if [ "${RACE_FAILED:-unset}" != "0" ]; then
    echo "FAIL: (c) race-signature output serialized FAILED=${RACE_FAILED:-unset} (expected 0 — phantom exec-failure count must NOT be parsed)"
    exit 1
fi
echo "OK: (c) race-signature output yields FAILED=0 (phantom count not serialized)"

# (d) NEGATIVE PIN — clean output with real failures stays reported
clean_fix="$(mktemp)"
cat > "$clean_fix" <<'EOF'
    PASS [   0.020s] (1/5) ori_llvm::aot collections_ext::test_a
    FAIL [   0.020s] (2/5) ori_llvm::aot collections_ext::test_b
    Summary [  12.0s] 5 tests run: 3 passed, 2 failed, 0 skipped
EOF
NEXTEST_ACTIVE=1 parse_rust_results "$clean_fix" "CLEAN"
rm -f "$clean_fix"
if [ "${CLEAN_FAILED:-unset}" != "2" ]; then
    echo "FAIL: (d) clean nextest output with real failures reported FAILED=${CLEAN_FAILED:-unset} (expected 2 — genuine failures must NOT be masked)"
    exit 1
fi
echo "OK: (d) clean output with real failures still reports FAILED=2 (no masking)"

# (e) SQUEEZE PIN — mixed real FAIL + race exec-failure => FAILED=0
mixed_fix="$(mktemp)"
cat > "$mixed_fix" <<EOF
    PASS [   0.020s] (1/5) ori_llvm::aot collections_ext::test_a
    FAIL [   0.020s] (2/5) ori_llvm::aot collections_ext::test_b
    $RACE_LINE
    Summary [  12.0s] 5 tests run: 2 passed, 3 failed, 0 skipped
EOF
NEXTEST_ACTIVE=1 parse_rust_results "$mixed_fix" "MIXED"
rm -f "$mixed_fix"
if [ "${MIXED_FAILED:-unset}" != "0" ]; then
    echo "FAIL: (e) mixed real+exec output reported FAILED=${MIXED_FAILED:-unset} (expected 0 — a race-contaminated Summary cannot separate real from exec failures; classify whole-leg errored)"
    exit 1
fi
echo "OK: (e) mixed real+exec output yields FAILED=0 (whole-leg errored; aggregate Summary not trusted)"

# (f) suite_status integration
race_status="$(suite_status 1 0)"
if [ "$race_status" != "errored" ]; then
    echo "FAIL: (f) suite_status(exit=1, failed=0) returned '$race_status' (expected 'errored' — race leg must be a non-verdict)"
    exit 1
fi
real_status="$(suite_status 1 2)"
if [ "$real_status" != "failed" ]; then
    echo "FAIL: (f) suite_status(exit=1, failed=2) returned '$real_status' (expected 'failed' — real failures must stay a verdict)"
    exit 1
fi
echo "OK: (f) suite_status — (nonzero,0)=>errored non-verdict, (nonzero,2)=>failed verdict"

# (g) GUARD — unset BUILD_RACE_RE must not turn the race grep into a
# match-all (empty ERE matches every non-empty line), which would
# misclassify a genuinely clean nextest Summary as a race.
saved_build_race_re="$BUILD_RACE_RE"
unset BUILD_RACE_RE
noguard_fix="$(mktemp)"
cat > "$noguard_fix" <<'EOF'
    PASS [   0.020s] (1/3) ori_stack tests::grows
     Summary [   0.020s] 3 tests run: 3 passed, 0 skipped
EOF
NEXTEST_ACTIVE=1 parse_rust_results "$noguard_fix" "NOGUARD"
rm -f "$noguard_fix"
BUILD_RACE_RE="$saved_build_race_re"
if [ "${NOGUARD_PASSED:-unset}" != "3" ] || [ "${NOGUARD_FAILED:-unset}" != "0" ]; then
    echo "FAIL: (g) unset BUILD_RACE_RE misclassified a clean Summary as a race: PASSED=${NOGUARD_PASSED:-unset} FAILED=${NOGUARD_FAILED:-unset} (expected 3/0 — an empty ERE pattern must never match-all)"
    exit 1
fi
echo "OK: (g) unset BUILD_RACE_RE does not misclassify a clean Summary as a race"

echo "PASS: test_test_all_nextest_race — race exec-failures classified errored (non-verdict), real failures preserved"
