#!/usr/bin/env bash
# Regression: ORI_TEST_FORCE_FULL was read directly inline in test-all.sh
# with no test asserting it actually changes INCREMENTAL_ARGS (the flag
# passed to the interpreter + LLVM Ori spec-test runner legs) and no
# documentation of the flag anywhere a caller would find it. Cure: the
# flag-selection logic is extracted into legs.sh:compute_incremental_args (a
# testable function both test-all.sh and this pin consume), and documented
# in test-all.sh's own usage header (this repo's sole doc surface for its
# own CLI/env-var contract).
#
# Pins:
#  (a) DEFAULT PATH - unset ORI_TEST_FORCE_FULL -> INCREMENTAL_ARGS carries
#      exactly --incremental.
#  (b) FORCE-FULL PATH - ORI_TEST_FORCE_FULL=1 -> INCREMENTAL_ARGS is empty
#      (the runner gets no --incremental flag -> every target executes).
#  (c) SSOT WIRING - test-all.sh calls compute_incremental_args (not a
#      re-inlined copy of the if/else).
#  (d) DOC - test-all.sh's own usage header documents ORI_TEST_FORCE_FULL.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LEG_HELPERS="$HERE/../test_all/legs.sh"
TESTALL="$HERE/../../test-all.sh"

if [ ! -f "$LEG_HELPERS" ]; then
    echo "FAIL: leg helper file not found at $LEG_HELPERS"
    exit 1
fi
if [ ! -f "$TESTALL" ]; then
    echo "FAIL: test-all.sh not found at $TESTALL"
    exit 1
fi

# shellcheck source=/dev/null
. "$LEG_HELPERS"

if ! declare -F compute_incremental_args >/dev/null 2>&1; then
    echo "FAIL: compute_incremental_args not defined in $LEG_HELPERS"
    exit 1
fi

# (a) DEFAULT PATH
unset ORI_TEST_FORCE_FULL 2>/dev/null || true
INCREMENTAL_ARGS=()
compute_incremental_args >/dev/null
if [ "${#INCREMENTAL_ARGS[@]}" -ne 1 ] || [ "${INCREMENTAL_ARGS[0]}" != "--incremental" ]; then
    echo "FAIL: (a) default (unset ORI_TEST_FORCE_FULL) INCREMENTAL_ARGS='${INCREMENTAL_ARGS[*]:-}' (expected exactly --incremental)"
    exit 1
fi
echo "OK: (a) unset ORI_TEST_FORCE_FULL -> INCREMENTAL_ARGS=(--incremental)"

# (b) FORCE-FULL PATH
# shellcheck disable=SC2034 # read by compute_incremental_args (sourced from legs.sh)
ORI_TEST_FORCE_FULL=1
INCREMENTAL_ARGS=(--incremental)
compute_incremental_args >/dev/null
if [ "${#INCREMENTAL_ARGS[@]}" -ne 0 ]; then
    echo "FAIL: (b) ORI_TEST_FORCE_FULL=1 INCREMENTAL_ARGS='${INCREMENTAL_ARGS[*]:-}' (expected empty array)"
    exit 1
fi
unset ORI_TEST_FORCE_FULL
echo "OK: (b) ORI_TEST_FORCE_FULL=1 -> INCREMENTAL_ARGS=() (no --incremental)"

# (c) SSOT WIRING - test-all.sh calls the function, not a re-inlined copy
if ! grep -qE '^compute_incremental_args$' "$TESTALL"; then
    echo "FAIL: (c) test-all.sh does not call compute_incremental_args (flag-selection logic re-inlined or dropped)"
    exit 1
fi
if grep -qE '^INCREMENTAL_ARGS=\(--incremental\)$' "$TESTALL"; then
    echo "FAIL: (c) test-all.sh still carries the old inline INCREMENTAL_ARGS assignment (duplicate of legs.sh SSOT)"
    exit 1
fi
echo "OK: (c) test-all.sh consumes the legs.sh SSOT function (no re-inlined copy)"

# (d) DOC - test-all.sh's own usage header documents the flag (its sole
# in-repo doc surface; a wrapper-side rules file is NOT part of this public
# repo and must never be referenced from it).
header="$(sed -n '1,20p' "$TESTALL")"
if ! printf '%s' "$header" | grep -qF 'ORI_TEST_FORCE_FULL'; then
    echo "FAIL: (d) test-all.sh's usage header does not document ORI_TEST_FORCE_FULL"
    exit 1
fi
echo "OK: (d) test-all.sh's usage header documents ORI_TEST_FORCE_FULL"

echo "PASS: test_test_all_incremental_force_full - ORI_TEST_FORCE_FULL changes INCREMENTAL_ARGS, wired through the legs.sh SSOT, documented"
