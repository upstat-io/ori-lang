#!/bin/bash
# Run ALL tests: Rust unit tests and Ori language tests
# Usage: ./test-all.sh [-v|--verbose] [-s|--sequential] [--json[=<path>]] [--json-summary[=<path>]]
#
# This script runs:
# 1. Rust unit tests (workspace default members - excludes ori_llvm)
# 2. Rust doctests (workspace + ori_llvm)
# 3. Runtime library tests (ori_rt)
# 4. Rust unit tests (ori_llvm)
# 5. AOT integration tests (compile-and-run through ori build)
# 6. External playground WASM build (cargo build of ori-lang-website crate)
# 7. Ori language spec tests (interpreter backend)
# 8. Ori language spec tests (LLVM backend)
#
# By default, runs tests in parallel for faster execution.
# Use -s or --sequential for sequential execution.
# Use -v or --verbose to see all output.
# Set ORI_TEST_FORCE_FULL=1 to force a full Ori spec-test run (skips the
# --incremental unchanged-target optimization for the interpreter + LLVM legs).

set -e

# Force color off for every cargo/nextest invocation: captured leg output is
# parsed by scripts/test_all/parsing.sh via exact-text matching, and callers
# with FORCE_COLOR set would otherwise inject ANSI sequences into the capture.
export CARGO_TERM_COLOR=never
export NEXTEST_COLOR=never

# AOT identity-gate library: snapshot-integrity verdict + manifest helpers.
# Sourced by BASH_SOURCE-relative path so it resolves regardless of cwd.
TEST_ALL_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/aot_gate_lib.sh
source "$TEST_ALL_DIR/scripts/aot_gate_lib.sh"

# Private bounded-action entry point for scripts.test_all_runtime.
RUNTIME_ACTION=""
RUNTIME_DIR=""
RUNTIME_RUN_ID=""
RUNTIME_PLAN_DIGEST=""
for arg in "$@"; do
    case $arg in
        --runtime-action=*) RUNTIME_ACTION="${arg#--runtime-action=}" ;;
        --runtime-dir=*) RUNTIME_DIR="${arg#--runtime-dir=}" ;;
        --runtime-run-id=*) RUNTIME_RUN_ID="${arg#--runtime-run-id=}" ;;
        --runtime-plan-digest=*) RUNTIME_PLAN_DIGEST="${arg#--runtime-plan-digest=}" ;;
    esac
done
if [ -n "$RUNTIME_ACTION" ]; then
    if [ -z "$RUNTIME_DIR" ] || [ -z "$RUNTIME_RUN_ID" ] || [ -z "$RUNTIME_PLAN_DIGEST" ]; then
        echo "runtime action requires --runtime-dir, --runtime-run-id, and --runtime-plan-digest" >&2
        exit 2
    fi
    if [ "${ORI_TESTALL_RUN_ID:-}" != "$RUNTIME_RUN_ID" ] || [ "${ORI_TESTALL_PLAN_DIGEST:-}" != "$RUNTIME_PLAN_DIGEST" ]; then
        echo "runtime action identity does not match ORI_TESTALL_RUN_ID/ORI_TESTALL_PLAN_DIGEST" >&2
        exit 2
    fi
    case "$RUNTIME_DIR" in
        "$TEST_ALL_DIR"/build/test-all-runs/"$RUNTIME_RUN_ID") ;;
        *) echo "runtime directory is outside the reserved run directory" >&2; exit 2 ;;
    esac
    # shellcheck source=scripts/test_all/legs.sh
    source "$TEST_ALL_DIR/scripts/test_all/legs.sh"
    # shellcheck source=scripts/test_all/parsing.sh
    source "$TEST_ALL_DIR/scripts/test_all/parsing.sh"
    # shellcheck source=scripts/test_all/post_run.sh
    source "$TEST_ALL_DIR/scripts/test_all/post_run.sh"
    # shellcheck source=scripts/test_all/runtime_actions.sh
    source "$TEST_ALL_DIR/scripts/test_all/runtime_actions.sh"
    runtime_action_main
    exit $?
fi

ACTIVE_RUNTIME="$TEST_ALL_DIR/build/test-all-runs/active.json"
if [ -f "$ACTIVE_RUNTIME" ]; then
    if ! ACTIVE_FIELDS=$(timeout 10 python3 - "$ACTIVE_RUNTIME" <<'PY' 2>/dev/null
import json, sys
doc = json.load(open(sys.argv[1], encoding="utf-8"))
print(f"{doc.get('run_id', '')} {doc.get('phase', '')}")
PY
    ); then
        echo "active test-all reservation is unreadable at $ACTIVE_RUNTIME; repair or remove it before a legacy run" >&2
        exit 2
    fi
    read -r ACTIVE_RUN_ID ACTIVE_PHASE <<< "$ACTIVE_FIELDS"
    case "$ACTIVE_PHASE" in
        planning|ready|running|resume_required|finalizing)
            echo "test-all run '$ACTIVE_RUN_ID' is resumable; use: python -m scripts.test_all_runtime run --run-id $ACTIVE_RUN_ID" >&2
            exit 2
            ;;
    esac
fi

# Check for flags
VERBOSE=0
PARALLEL=1
EMIT_JSON=0
JSON_PATH=""
EMIT_JSON_SUMMARY=0
JSON_SUMMARY_PATH=""
for arg in "$@"; do
    case $arg in
        -v|--verbose)
            VERBOSE=1
            ;;
        -s|--sequential)
            PARALLEL=0
            ;;
        --json)
            EMIT_JSON=1
            JSON_PATH="website/public/test-results.json"
            ;;
        --json=*)
            EMIT_JSON=1
            JSON_PATH="${arg#--json=}"
            ;;
        --json-summary)
            EMIT_JSON_SUMMARY=1
            JSON_SUMMARY_PATH="/tmp/test-summary-$$.json"
            ;;
        --json-summary=*)
            EMIT_JSON_SUMMARY=1
            JSON_SUMMARY_PATH="${arg#--json-summary=}"
            ;;
    esac
done

# Per-run build isolation. Concurrent runs sharing target/ rebuild each
# other's artifacts mid-suite and mass-fail the AOT leg; ORI_TESTALL_BUILD_ID
# isolates a caller's target/<build-id> (default "shared"). Full-suite
# verdicts still serialize globally via a lock outside target/, so
# cargo clean / cache cleanup cannot unlink the active lock's inode.
TESTALL_BUILD_ID="${ORI_TESTALL_BUILD_ID:-shared}"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$(pwd)/target/test-all-${TESTALL_BUILD_ID}}"
TARGET_DIR="$CARGO_TARGET_DIR"
TESTALL_LOCK_DIR="$TEST_ALL_DIR/build/test-all-locks"
mkdir -p "$TARGET_DIR" "$TESTALL_LOCK_DIR"
TESTALL_GLOBAL_LOCK="$TESTALL_LOCK_DIR/.test-all-global.lock"
if [ -z "${ORI_TESTALL_GLOBAL_FLOCKED:-}" ] && command -v flock >/dev/null 2>&1; then
    if ! flock --nonblock "$TESTALL_GLOBAL_LOCK" true 2>/dev/null; then
        echo "waiting for another test-all run to finish..."
    fi
    export ORI_TESTALL_GLOBAL_FLOCKED=1
    exec flock --close "$TESTALL_GLOBAL_LOCK" "$0" "$@"
fi
TESTALL_LOCK="$TESTALL_LOCK_DIR/.test-all-${TESTALL_BUILD_ID}.lock"
if [ -z "${ORI_TESTALL_FLOCKED:-}" ] && command -v flock >/dev/null 2>&1; then
    if ! flock --nonblock "$TESTALL_LOCK" true 2>/dev/null; then
        echo "waiting for a concurrent test-all run with build id '$TESTALL_BUILD_ID' to finish..."
    fi
    export ORI_TESTALL_FLOCKED=1
    exec flock --close "$TESTALL_LOCK" "$0" "$@"
fi
# Re-exec inherits CARGO_TARGET_DIR via export; recompute the convenience var.
TARGET_DIR="$CARGO_TARGET_DIR"

# Always log full output to a fixed file (cleared on each run)
LOG_FILE="test-all.log"
: > "$LOG_FILE"
exec > >(tee -a "$LOG_FILE") 2>&1
echo "LOGGING ALL OUTPUT TO $(pwd)/$LOG_FILE"
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BOLD='\033[1m'
NC='\033[0m'

# Temp files for capturing output
RUST_OUTPUT=$(mktemp)
RUST_RT_OUTPUT=$(mktemp)
RUST_LLVM_OUTPUT=$(mktemp)
DOCTEST_OUTPUT=$(mktemp)
AOT_OUTPUT=$(mktemp)
WASM_OUTPUT=$(mktemp)
ORI_INTERP_OUTPUT=$(mktemp)
ORI_LLVM_OUTPUT=$(mktemp)
# Per-leg wall-time capture dir: each timed_leg writes "<secs>" to
# $LEG_TIMING_DIR/<name>. Additive observability only - no pass/fail effect.
LEG_TIMING_DIR=$(mktemp -d)
# Machine-readable runner JSON (`ori test --format json`) per backend, written
# to disc-backed files (single run per backend; the console summary + failure
# detail are reconstructed from these via diagnostics/parse_test_json.py - no
# second runner invocation). $ORI_*_OUTPUT capture the runner's stderr so a
# crash diagnostic survives even when stdout carries no parseable JSON.
ORI_INTERP_JSON="$(dirname "$0")/build/ori-interp-results.json"
ORI_LLVM_JSON="$(dirname "$0")/build/ori-llvm-results.json"
PARSE_TEST_JSON="$(dirname "$0")/diagnostics/parse_test_json.py"
PER_NODE_VERDICT="$(dirname "$0")/diagnostics/per_node_verdict.py"

# Stale runner-JSON from a prior run would mask a current build-failure (the
# -f guard at parse time would parse last run's results). Remove up front so a
# present file always means THIS run produced it.
rm -f "$ORI_INTERP_JSON" "$ORI_LLVM_JSON"

# Cleanup temp files on exit
# shellcheck disable=SC2317 # invoked via `trap cleanup EXIT`, not a direct call shellcheck can see
cleanup() {
    rm -f "$RUST_OUTPUT" "$RUST_RT_OUTPUT" "$RUST_LLVM_OUTPUT" "$DOCTEST_OUTPUT" "$AOT_OUTPUT" "$WASM_OUTPUT" "$ORI_INTERP_OUTPUT" "$ORI_LLVM_OUTPUT"
    rm -rf "$LEG_TIMING_DIR"
}
trap cleanup EXIT

# Track failures
RUST_EXIT=0
DOCTEST_EXIT=0
RUST_RT_EXIT=0
RUST_LLVM_EXIT=0
AOT_EXIT=0
WASM_EXIT=0
ORI_INTERP_EXIT=0
ORI_LLVM_EXIT=0

# Verification gates
# ARC IR verification: checks RC balance, drop placement after AIMS pipeline.
# Also enables per-function LLVM IR verification at all emission sites.
export ORI_VERIFY_ARC=1
# LLVM pass verification: verifies IR well-formedness after every optimization pass.
# Measured overhead: ~20% wall time (54s vs ~45s baseline), within 150s budget.
export ORI_VERIFY_EACH=1

# Test-all helper libraries
# shellcheck source=scripts/test_all/legs.sh
source "$TEST_ALL_DIR/scripts/test_all/legs.sh"
# shellcheck source=scripts/test_all/parsing.sh
source "$TEST_ALL_DIR/scripts/test_all/parsing.sh"
# shellcheck source=scripts/test_all/json_report.sh
source "$TEST_ALL_DIR/scripts/test_all/json_report.sh"
# shellcheck source=scripts/test_all/post_run.sh
source "$TEST_ALL_DIR/scripts/test_all/post_run.sh"
# shellcheck source=scripts/test_all/reporting.sh
source "$TEST_ALL_DIR/scripts/test_all/reporting.sh"

# Main execution

# Flag consistency check (blocking - abort on failure)
echo "=== Checking debug flag consistency ==="
if diagnostics/check-debug-flags.sh --no-color > /dev/null 2>&1; then
    echo "  [ok] Debug flag consistency check passed"
else
    echo -e "${RED}  [fail] Debug flag consistency check FAILED${NC}"
    echo "    Run 'diagnostics/check-debug-flags.sh' for details."
    exit 1
fi
echo ""

# Crate-DAG + canon-consumer registration lint (--warn-only): surfaces
# unregistered-consumer source-scan findings without gating the suite.
echo "=== Checking crate-DAG + canon-consumer registration ==="
python3 scripts/crate-dag-lint.py --warn-only
echo "  [ok] crate-DAG + canon-consumer registration lint (bedding-in --warn-only)"
echo ""

# Harness self-tests (blocking - abort on failure): each pins a specific
# test-all.sh correctness property (ANSI-summary parsing, build-race
# classification, prebuild-shape parity, JSON escaping, exit-status
# semantics, AOT snapshot-integrity gate) that every downstream verdict
# this run produces depends on, so they gate before any test leg runs.
echo "=== Running test-all.sh harness self-tests ==="
HARNESS_SELFTEST_FAILED=0
HARNESS_SELFTEST_LOG=$(mktemp)
for _selftest in "$TEST_ALL_DIR"/scripts/tests/*.sh; do
    [ -f "$_selftest" ] || continue
    if bash "$_selftest" > "$HARNESS_SELFTEST_LOG" 2>&1; then
        echo "  [ok] $(basename "$_selftest")"
    else
        echo -e "${RED}  [fail] $(basename "$_selftest") FAILED${NC}"
        cat "$HARNESS_SELFTEST_LOG"
        HARNESS_SELFTEST_FAILED=1
    fi
done
rm -f "$HARNESS_SELFTEST_LOG"
if [ "$HARNESS_SELFTEST_FAILED" -eq 1 ]; then
    echo -e "${RED}  [fail] test-all.sh harness self-tests FAILED (see above)${NC}"
    exit 1
fi
echo ""

# Build cache: wrap this run's cargo builds with sccache only when the
# wrapper can actually execute rustc — an unusable sccache binary/socket
# treated as ready would turn a cache issue into a false suite failure.
if command -v sccache >/dev/null 2>&1; then
    SCCACHE_PREFLIGHT_OUTPUT=$(mktemp)
    if timeout 10 sccache rustc -vV > "$SCCACHE_PREFLIGHT_OUTPUT" 2>&1; then
        export RUSTC_WRAPPER=sccache
        # sccache cannot cache incrementally-compiled crates; debug builds default to
        # CARGO_INCREMENTAL=1, so without this the wrapper caches nothing (compile
        # requests executed = 0). Disabling cargo incremental lets sccache cache + hit.
        export CARGO_INCREMENTAL=0
        echo "=== Build cache: sccache active (RUSTC_WRAPPER=sccache, CARGO_INCREMENTAL=0) ==="
    else
        unset RUSTC_WRAPPER
        echo "=== Build cache: sccache unusable - builds proceed unwrapped ==="
        sed -n '1,3p' "$SCCACHE_PREFLIGHT_OUTPUT" | sed 's/^/  sccache preflight: /'
    fi
    rm -f "$SCCACHE_PREFLIGHT_OUTPUT"
else
    echo "=== Build cache: sccache absent - builds proceed unwrapped ==="
fi

# Fast linker (mold), scoped to this run's RUSTFLAGS only (never a global
# .cargo/config.toml change) and guarded on `command -v mold` — -fuse-ld=mold
# errors at link time if mold is absent, so the flag is added only when
# detected; absent -> default linker.
if command -v mold >/dev/null 2>&1; then
    export RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }-C link-arg=-fuse-ld=mold"
    echo "=== Fast linker: mold active (-fuse-ld=mold) ==="
else
    echo "=== Fast linker: mold absent - default linker ==="
fi

# Test runner: route the Rust legs through cargo-nextest (process-per-test
# scheduling) when on PATH, else `cargo test` via cargo_race_retry; absent ->
# NEXTEST_ACTIVE empty. Phase 1 pre-builds nextest binaries with `--no-run`
# so Phase-2 legs never recompile; doctests always stay on `cargo test --doc`
# (nextest cannot run them).
NEXTEST_ACTIVE=""
if command -v cargo-nextest >/dev/null 2>&1; then
    NEXTEST_ACTIVE=1
    echo "=== Test runner: cargo-nextest active (Rust legs) ==="
else
    echo "=== Test runner: cargo-nextest absent - cargo test legs ==="
fi

# Incremental spec-test skip: pass --incremental to the Ori spec-test runner
# so it skips targets unchanged since the last run; ORI_TEST_FORCE_FULL forces
# a full run instead.
compute_incremental_args
echo ""

if [[ $PARALLEL -eq 1 ]]; then
    echo -e "${BOLD}Running tests in parallel...${NC}"
    echo ""

    # Phase 1: serial pre-builds cover every artifact the run phase needs.
    # Each Phase-2 leg's EXACT cargo selection is warmed first (a mismatched
    # shape recompiles concurrently into the shared target/, racing the
    # other legs); after this phase every run invocation is pure test
    # execution (run_rust_doctests is the lone exception — rustdoc compiles
    # at run time as Phase 2's sole remaining workspace-crate compiler).
    echo "=== Pre-building all debug test artifacts (serial, race-free) ==="
    _PHASE1_T0=$(date +%s.%N)
    if ! prebuild_leg_shapes; then
        echo -e "  ${RED}[fail] per-leg test-binary pre-build FAILED (see above)${NC}"
    else
        echo "  [ok] Per-leg test binaries pre-built (all leg selection shapes)"
    fi

    # ori bin + libori_rt.a staticlib (cargo test --no-run builds the rlib,
    # not the staticlib; the interpreter suite + AOT links need these).
    # Single-package shapes matching the AOT harness's own per-process
    # builds exactly - a joint `-p oric -p ori_rt` selection unifies
    # features differently, leaving the harness's builds as unwarmed
    # Phase-2 writers into the shared target/.
    if ! build_with_race_retry cargo build -p ori_rt -q; then
        echo -e "  ${RED}[fail] Debug ori_rt build FAILED${NC}"
    fi
    if ! build_with_race_retry cargo build -p oric --bin ori -q; then
        echo -e "  ${RED}[fail] Debug ori build FAILED${NC}"
    fi

    # LLVM release build (sequential - shares target/ with the debug build).
    echo "=== Building LLVM release binary ==="
    LLVM_BUILD_OK=1
    if ! build_with_race_retry cargo build -p ori_rt --release -q; then
        echo -e "  ${RED}[fail] LLVM release ori_rt build FAILED - skipping LLVM spec tests${NC}"
        LLVM_BUILD_OK=0
        ORI_LLVM_EXIT=1
    fi
    if [[ $LLVM_BUILD_OK -eq 1 ]] && ! build_with_race_retry cargo build -p oric --bin ori --release -q; then
        echo -e "  ${RED}[fail] LLVM release build FAILED - skipping LLVM spec tests${NC}"
        LLVM_BUILD_OK=0
        ORI_LLVM_EXIT=1
    fi
    awk "BEGIN { printf \"%.1f\", $(date +%s.%N) - ${_PHASE1_T0} }" > "$LEG_TIMING_DIR/phase1_build" 2>/dev/null

    echo ""

    # Phase 2: the workspace crate graph was pre-built in Phase 1, so those
    # invocations only RUN their binaries - no concurrent compile of workspace
    # crates into the shared target/, hence no build-artifact race for them.
    # - run_rust_workspace: --workspace --exclude ori_llvm --lib --bins --tests (run pre-built)
    # - run_rust_doctests: --workspace --doc (the lone workspace-crate compiling invocation)
    # - run_rust_rt: -p ori_rt (run pre-built)
    # - run_rust_llvm: -p ori_llvm --lib (run pre-built)
    # - run_aot: -p ori_llvm --test aot (run pre-built)
    # - run_ori_interpreter: direct binary ($CARGO_TARGET_DIR/debug/ori), no cargo
    # - run_ori_llvm: direct binary ($CARGO_TARGET_DIR/release/ori), no cargo
    # - run_wasm_build: a disjoint crate graph on a disjoint target-triple
    #   subdir (website playground workspace) inside the SAME $CARGO_TARGET_DIR
    #   root - shares the root but cannot collide with the workspace-crate
    #   artifacts the other legs read/write
    # AOT identity-gate baseline (Go model: pin at start, immune thereafter).
    # The harness hardlink-stages the compiler binary + staticlib per-process
    # into $AOT_STAGE_MANIFEST; the post-leg gate validates that manifest, so
    # later target/ churn is harmless. Delete any stale manifest first.
    rm -f "$AOT_STAGE_MANIFEST"
    AOT_ARTIFACT_BASELINE=$(artifact_identity)

    timed_leg rust_workspace run_rust_workspace &
    RUST_PID=$!

    timed_leg rust_doctests run_rust_doctests &
    DOCTEST_PID=$!

    timed_leg wasm_build run_wasm_build &
    WASM_PID=$!

    timed_leg rust_rt run_rust_rt &
    RUST_RT_PID=$!

    timed_leg rust_llvm run_rust_llvm &
    RUST_LLVM_PID=$!

    timed_leg aot run_aot &
    AOT_PID=$!

    timed_leg ori_interpreter run_ori_interpreter &
    ORI_INTERP_PID=$!

    if [[ $LLVM_BUILD_OK -eq 1 ]]; then
        timed_leg ori_llvm run_ori_llvm &
        ORI_LLVM_PID=$!
    fi

    wait "$RUST_PID" || RUST_EXIT=1
    wait "$DOCTEST_PID" || DOCTEST_EXIT=1
    wait "$WASM_PID" || WASM_EXIT=1
    wait "$RUST_RT_PID" || RUST_RT_EXIT=1
    wait "$RUST_LLVM_PID" || RUST_LLVM_EXIT=1
    wait "$AOT_PID" || AOT_EXIT=1
    ORI_INTERP_EXIT=0
    wait "$ORI_INTERP_PID" || ORI_INTERP_EXIT=$?
    if [[ $LLVM_BUILD_OK -eq 1 ]]; then
        ORI_LLVM_EXIT=0
        wait "$ORI_LLVM_PID" || ORI_LLVM_EXIT=$?
    fi

else
    # Sequential execution
    echo -e "${BOLD}Running tests sequentially...${NC}"
    echo ""

    timed_leg rust_workspace run_rust_workspace || RUST_EXIT=1
    echo ""
    echo "=== Building runtime library (debug) ==="
    if ! build_with_race_retry cargo build -p ori_rt -q; then
        echo -e "  ${RED}[fail] Runtime library debug build FAILED${NC}"
    fi
    if ! build_with_race_retry cargo build -p oric --bin ori -q; then
        echo -e "  ${RED}[fail] Debug ori build FAILED${NC}"
    fi
    echo "=== Building LLVM release binary ==="
    LLVM_BUILD_OK=1
    if ! build_with_race_retry cargo build -p ori_rt --release -q; then
        echo -e "  ${RED}[fail] LLVM release ori_rt build FAILED - skipping LLVM spec tests${NC}"
        LLVM_BUILD_OK=0
        ORI_LLVM_EXIT=1
    fi
    if [[ $LLVM_BUILD_OK -eq 1 ]] && ! build_with_race_retry cargo build -p oric --bin ori --release -q; then
        echo -e "  ${RED}[fail] LLVM release build FAILED - skipping LLVM spec tests${NC}"
        LLVM_BUILD_OK=0
        ORI_LLVM_EXIT=1
    fi
    echo ""
    timed_leg rust_rt run_rust_rt || RUST_RT_EXIT=1
    echo ""
    timed_leg rust_llvm run_rust_llvm || RUST_LLVM_EXIT=1
    echo ""
    timed_leg rust_doctests run_rust_doctests || DOCTEST_EXIT=1
    echo ""
    # AOT identity-gate baseline (Go model: pin at start, immune thereafter);
    # delete any stale manifest first so post-leg presence proves THIS run staged.
    rm -f "$AOT_STAGE_MANIFEST"
    AOT_ARTIFACT_BASELINE=$(artifact_identity)
    timed_leg aot run_aot || AOT_EXIT=1
    echo ""
    timed_leg wasm_build run_wasm_build || WASM_EXIT=1
    echo ""
    ORI_INTERP_EXIT=0
    timed_leg ori_interpreter run_ori_interpreter || ORI_INTERP_EXIT=$?
    echo ""
    if [[ $LLVM_BUILD_OK -eq 1 ]]; then
        ORI_LLVM_EXIT=0
        timed_leg ori_llvm run_ori_llvm || ORI_LLVM_EXIT=$?
    fi
fi

print_leg_timings
apply_aot_snapshot_verdict
show_suite_outputs
collect_suite_results
compute_suite_statuses
print_test_summary
finalize_test_all
