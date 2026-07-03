#!/usr/bin/env bash
# TDD pin: test-all.sh must exit red for every failed suite, build error, or
# crash. A crashed LLVM spec leg is part of ANY_FAILED and must not get a green
# final status when the other suites pass.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TESTALL="$HERE/../../test-all.sh"
REPORTING_HELPERS="$HERE/../test_all/reporting.sh"

if [ ! -f "$TESTALL" ]; then
    echo "FAIL: test-all.sh not found at $TESTALL"
    exit 1
fi
if [ ! -f "$REPORTING_HELPERS" ]; then
    echo "FAIL: reporting helper file not found at $REPORTING_HELPERS"
    exit 1
fi

fn="$(sed -n '/^test_all_final_exit_code()/,/^}/p' "$REPORTING_HELPERS")"
if [ -z "$fn" ]; then
    echo "FAIL: test_all_final_exit_code() is not defined in test-all.sh"
    exit 1
fi
eval "$fn"

if test_all_final_exit_code 0; then
    echo "OK: zero failures exits green"
else
    echo "FAIL: zero failures returned red"
    exit 1
fi

if test_all_final_exit_code 1; then
    echo "FAIL: one failed suite returned green"
    exit 1
else
    echo "OK: one failed suite exits red"
fi

if grep -q 'All other suites passed' "$TESTALL"; then
    echo "FAIL: LLVM crash exemption message is still present"
    exit 1
fi

if grep -q 'ORI_LLVM_CRASHED.*exit 0' "$TESTALL"; then
    echo "FAIL: ORI_LLVM_CRASHED still has a green-exit branch"
    exit 1
fi

echo "PASS: test_test_all_final_status - any failed suite exits red"
