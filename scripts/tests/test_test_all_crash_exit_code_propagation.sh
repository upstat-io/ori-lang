#!/usr/bin/env bash
# Regression: run_ori_interpreter / run_ori_llvm always `return 1` on both a
# CRASH (exit code > 128, e.g. SIGSEGV) and an ordinary test FAILURE,
# discarding the crash-distinguishing exit code before it reaches
# parse_ori_results' `exit_code -gt 128` branch. The caller's ORI_INTERP_EXIT /
# ORI_LLVM_EXIT is bounded to {0,1} via timed_leg/wait, so parse_ori_results'
# CRASHED detection was unreachable dead code: a genuine crash was reported as
# a plain FAILED with 0/0/0/0 counts instead of CRASHED, corrupting
# reporting.sh + json_report.sh's status display and post_run.sh's
# INCOMPLETE_SUITES counter (all keyed on <PREFIX>_CRASHED).
#
# Pins:
#  (a) run_ori_interpreter returns the real (>128) exit code on a crash, not 1.
#  (b) run_ori_llvm returns the real (>128) exit code on a crash, not 1.
#  (c) end-to-end: parse_ori_results, fed that real exit code, sets
#      <PREFIX>_CRASHED=1 (the flag reporting.sh/json_report.sh/post_run.sh key on).
#  (d) an ordinary (non-crash, exit code 1) failure still returns non-zero and
#      is NOT misclassified as CRASHED.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LEG_HELPERS="$HERE/../test_all/legs.sh"
PARSING_HELPERS="$HERE/../test_all/parsing.sh"

if [ ! -f "$LEG_HELPERS" ]; then
    echo "FAIL: leg helper file not found at $LEG_HELPERS"
    exit 1
fi
if [ ! -f "$PARSING_HELPERS" ]; then
    echo "FAIL: parsing helper file not found at $PARSING_HELPERS"
    exit 1
fi

# shellcheck source=/dev/null
. "$LEG_HELPERS"
# shellcheck source=/dev/null
. "$PARSING_HELPERS"

SCRATCH="$(mktemp -d)"
cleanup() { rm -rf "$SCRATCH"; }
trap cleanup EXIT

mkdir -p "$SCRATCH/target/debug" "$SCRATCH/target/release"
# shellcheck disable=SC2034 # read by legs.sh:run_ori_interpreter/run_ori_llvm
CARGO_TARGET_DIR="$SCRATCH/target"
# shellcheck disable=SC2034 # read by legs.sh:run_ori_interpreter/run_ori_llvm
PARSE_TEST_JSON="$HERE/../../diagnostics/parse_test_json.py"
# shellcheck disable=SC2034 # read by legs.sh:run_ori_interpreter/run_ori_llvm
INCREMENTAL_ARGS=()

# --- (a) run_ori_interpreter propagates a real crash exit code -------------
# shellcheck disable=SC2034 # read by legs.sh:run_ori_interpreter
ORI_INTERP_JSON="$SCRATCH/interp.json"
# shellcheck disable=SC2034 # read by legs.sh:run_ori_interpreter
ORI_INTERP_OUTPUT="$SCRATCH/interp.out"
cat > "$SCRATCH/target/debug/ori" <<'EOF'
#!/usr/bin/env bash
echo "thread panicked" >&2
exit 139
EOF
chmod +x "$SCRATCH/target/debug/ori"

rc=0
run_ori_interpreter >/dev/null 2>&1 || rc=$?
if [ "$rc" -ne 139 ]; then
    echo "FAIL: (a) run_ori_interpreter returned $rc on a crashing binary (expected 139, the real signal-based exit code)"
    exit 1
fi
echo "OK: (a) run_ori_interpreter propagates the real (>128) crash exit code"

# --- (b) run_ori_llvm propagates a real crash exit code ---------------------
ORI_LLVM_JSON="$SCRATCH/llvm.json"
# shellcheck disable=SC2034 # read by legs.sh:run_ori_llvm
ORI_LLVM_OUTPUT="$SCRATCH/llvm.out"
cat > "$SCRATCH/target/release/ori" <<'EOF'
#!/usr/bin/env bash
echo "thread panicked" >&2
exit 139
EOF
chmod +x "$SCRATCH/target/release/ori"

rc=0
run_ori_llvm >/dev/null 2>&1 || rc=$?
if [ "$rc" -ne 139 ]; then
    echo "FAIL: (b) run_ori_llvm returned $rc on a crashing binary (expected 139, the real signal-based exit code)"
    exit 1
fi
echo "OK: (b) run_ori_llvm propagates the real (>128) crash exit code"

# --- (c) end-to-end: parse_ori_results sets <PREFIX>_CRASHED=1 on that code -
parse_ori_results "$ORI_LLVM_JSON" "ORI_LLVM" "$rc"
if [ "${ORI_LLVM_CRASHED:-0}" -ne 1 ]; then
    echo "FAIL: (c) parse_ori_results did not set ORI_LLVM_CRASHED=1 given the propagated crash exit code"
    exit 1
fi
echo "OK: (c) parse_ori_results sets <PREFIX>_CRASHED=1 given the real crash exit code"

# --- (d) an ordinary (non-crash) failure is not misclassified as CRASHED ----
cat > "$SCRATCH/target/release/ori" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' '{"files":[],"passed":0,"failed":1,"skipped":0,"skipped_unchanged":0,"llvm_compile_fail":0,"error_files":0,"llvm_compile_fail_files":0,"duration_ns":0}'
exit 1
EOF
chmod +x "$SCRATCH/target/release/ori"

rc=0
run_ori_llvm >/dev/null 2>&1 || rc=$?
if [ "$rc" -gt 128 ]; then
    echo "FAIL: (d) run_ori_llvm reported an ordinary failure as a crash exit code ($rc)"
    exit 1
fi
parse_ori_results "$ORI_LLVM_JSON" "ORI_LLVM" "$rc"
if [ "${ORI_LLVM_CRASHED:-0}" -ne 0 ]; then
    echo "FAIL: (d) an ordinary failure was misclassified as ORI_LLVM_CRASHED=1"
    exit 1
fi
echo "OK: (d) an ordinary (non-crash) failure is not misclassified as CRASHED"

# --- (e) production-path: the SAME timed_leg+background+wait invocation ----
# test-all.sh actually uses (`timed_leg NAME run_fn &` ... `wait "$PID" ||
# EXIT=$?`), under `set -euo pipefail` inherited into the background
# subshell. (a)-(d) above call run_ori_interpreter/run_ori_llvm directly as
# the left operand of `||`, which bash's errexit exemption rules suppress
# -e for the ENTIRE callee body - masking a real regression where the
# unprotected internal command inside run_ori_interpreter/run_ori_llvm trips
# errexit before the CRASHED-classification branch ever runs. This pin
# drives the real calling convention so that regression cannot hide again.
cat > "$SCRATCH/target/debug/ori" <<'EOF'
#!/usr/bin/env bash
echo "thread panicked at production-path repro" >&2
exit 139
EOF
chmod +x "$SCRATCH/target/debug/ori"

PRODPATH_OUT="$SCRATCH/prodpath.out"
(
    set -euo pipefail
    # shellcheck disable=SC2034 # read by legs.sh:run_ori_interpreter
    LEG_TIMING_DIR="$SCRATCH/leg-timing"
    mkdir -p "$LEG_TIMING_DIR"
    timed_leg ori_interpreter run_ori_interpreter &
    PID=$!
    PROD_EXIT=0
    wait "$PID" || PROD_EXIT=$?
    echo "PROD_EXIT=$PROD_EXIT"
) > "$PRODPATH_OUT" 2>&1

if ! grep -q "CRASHED: thread panicked at production-path repro" "$PRODPATH_OUT"; then
    echo "FAIL: (e) production-path invocation (timed_leg + background + wait) never printed the CRASHED diagnostic - set -e is short-circuiting the wrapped leg before its classification branch runs"
    cat "$PRODPATH_OUT"
    exit 1
fi
if ! grep -q "PROD_EXIT=139" "$PRODPATH_OUT"; then
    echo "FAIL: (e) production-path invocation did not propagate the real crash exit code (139)"
    cat "$PRODPATH_OUT"
    exit 1
fi
echo "OK: (e) production-path (timed_leg + background + wait, matching test-all.sh) surfaces the CRASHED diagnostic and propagates the real exit code"

echo "PASS: test_test_all_crash_exit_code_propagation - run_ori_interpreter/run_ori_llvm propagate real crash exit codes end-to-end into <PREFIX>_CRASHED"
