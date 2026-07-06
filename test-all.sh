#!/bin/bash
# Run ALL tests: Rust unit tests and Ori language tests
# Usage: ./test-all [-v|--verbose] [-s|--sequential] [--json[=<path>]]
#
# This script runs:
# 1. Rust unit tests (workspace default members - excludes ori_llvm)
# 2. Runtime library tests (ori_rt)
# 3. Rust unit tests (ori_llvm)
# 4. AOT integration tests (compile-and-run through ori build)
# 5. External playground WASM build (cargo build of ori-lang-website crate)
# 6. Ori language spec tests (interpreter backend)
# 7. Ori language spec tests (LLVM backend)
#
# By default, runs tests in parallel for faster execution.
# Use -s or --sequential for sequential execution.
# Use -v or --verbose to see all output.

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

# Per-run build isolation. Concurrent runs that share target/ rebuild each
# other's artifacts mid-suite and mass-fail the AOT leg with bogus failures. A
# caller can set ORI_TESTALL_BUILD_ID to isolate its target/<build-id>; absent
# that, runs use "shared". Full-suite verdicts are still serialized globally:
# running two complete suites at once makes the compiler-state verdict
# ambiguous and can race shared cargo/cache resources. Lock sentinels live
# outside target/ so cargo clean / cache cleanup cannot unlink the active lock
# path and let a second producer acquire a new inode.
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
> "$LOG_FILE"
exec > >(tee -a "$LOG_FILE") 2>&1
echo "LOGGING ALL OUTPUT TO $(pwd)/$LOG_FILE"
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BOLD='\033[1m'
NC='\033[0m' # No Color

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

# Build cache: wrap every cargo build below with sccache only when the wrapper
# can actually execute rustc. Some environments expose an unusable sccache
# binary/socket; treating mere presence as readiness turns a cache issue into a
# false suite failure before any tests run.
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

# Fast linker (mold) for this full-suite runner's cargo builds only, scoped to
# test-all.sh rather than a global .cargo/config.toml so it never changes how
# unrelated cargo invocations link. Guarded on `command -v mold`: -fuse-ld=mold
# errors at link if mold is absent (not a silent fallback), so the flag is added
# only when mold is detected; absent -> default linker.
if command -v mold >/dev/null 2>&1; then
    export RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }-C link-arg=-fuse-ld=mold"
    echo "=== Fast linker: mold active (-fuse-ld=mold) ==="
else
    echo "=== Fast linker: mold absent - default linker ==="
fi

# Test runner: route the Rust legs through cargo-nextest (intra-binary
# process-per-test scheduling) when it is on PATH, else fall back to plain
# `cargo test` via cargo_race_retry. Guarded on `command -v cargo-nextest`;
# absent -> NEXTEST_ACTIVE empty -> every leg uses the cargo_race_retry path
# unchanged. Phase 1 pre-builds the nextest binaries with `--no-run` so the
# Phase-2 run legs do not recompile (preserving the race-free pre-build/run
# split); doctests always stay on `cargo test --doc` (nextest cannot run them).
NEXTEST_ACTIVE=""
if command -v cargo-nextest >/dev/null 2>&1; then
    NEXTEST_ACTIVE=1
    echo "=== Test runner: cargo-nextest active (Rust legs) ==="
else
    echo "=== Test runner: cargo-nextest absent - cargo test legs ==="
fi

# Incremental spec-test skip: pass --incremental to the Ori spec-test runner so
# it skips targets unchanged since the last run (the runner already supports
# config.incremental for both the interpreter and LLVM legs). ORI_TEST_FORCE_FULL=1
# empties the flag to force a full run (every target executes).
INCREMENTAL_FLAG="--incremental"
if [ -n "${ORI_TEST_FORCE_FULL:-}" ]; then
    INCREMENTAL_FLAG=""
    echo "=== Incremental: ORI_TEST_FORCE_FULL set - full run (no --incremental) ==="
else
    echo "=== Incremental: --incremental active (unchanged targets skipped) ==="
fi
echo ""

if [[ $PARALLEL -eq 1 ]]; then
    echo -e "${BOLD}Running tests in parallel...${NC}"
    echo ""

    # Phase 1: ONE serial pre-build covering every debug artifact the run
    # phase needs - workspace test binaries (incl. ori_llvm lib/aot/codegen),
    # the ori bin, and libori_rt.a. A single cargo invocation parallelizes its
    # own job graph internally (optimal), and serializing it ahead of the run
    # phase prevents the shared-target/ build-artifact race (`failed to write
    # ...rmeta: No such file (os error 2)`) that concurrent compiling cargo
    # invocations trigger. After this phase, every run invocation is pure
    # test execution (the lone exception is run_rust_doctests - rustdoc
    # compiles at run time and runs as the single compiler in Phase 2).
    echo "=== Pre-building all debug test artifacts (serial, race-free) ==="
    _PHASE1_T0=$(date +%s.%N)
    PRECOMPILE_OUTPUT=$(mktemp)
    if cargo_race_retry "$PRECOMPILE_OUTPUT" cargo test --no-run -q --workspace --lib --bins --tests; then
        echo "  [ok] Debug test binaries pre-built"
    else
        echo -e "  ${RED}[fail] Debug test-binary pre-build FAILED${NC}"
        cat "$PRECOMPILE_OUTPUT"
    fi
    # nextest builds its OWN test harness binaries (distinct from cargo test's),
    # so when active they must ALSO be pre-built serially here - a Phase-2 plain
    # `cargo nextest run` would otherwise compile and reintroduce the rmeta race.
    if [ -n "$NEXTEST_ACTIVE" ]; then
        if cargo_race_retry "$PRECOMPILE_OUTPUT" cargo nextest run --no-run --workspace --lib --bins --tests; then
            echo "  [ok] nextest test binaries pre-built"
        else
            echo -e "  ${RED}[fail] nextest test-binary pre-build FAILED${NC}"
            cat "$PRECOMPILE_OUTPUT"
        fi
    fi
    rm -f "$PRECOMPILE_OUTPUT"

    # ori bin + libori_rt.a staticlib (cargo test --no-run builds the rlib,
    # not the staticlib; the interpreter suite + AOT links need these).
    if ! build_with_race_retry cargo build -p oric -p ori_rt -q; then
        echo -e "  ${RED}[fail] Debug ori/ori_rt build FAILED${NC}"
    fi

    # LLVM release build (sequential - shares target/ with the debug build).
    echo "=== Building LLVM release binary ==="
    LLVM_BUILD_OK=1
    if ! build_with_race_retry cargo build -p oric -p ori_rt --release -q; then
        echo -e "  ${RED}[fail] LLVM release build FAILED - skipping LLVM spec tests${NC}"
        LLVM_BUILD_OK=0
        ORI_LLVM_EXIT=1
    fi
    awk "BEGIN { printf \"%.1f\", $(date +%s.%N) - ${_PHASE1_T0} }" > "$LEG_TIMING_DIR/phase1_build" 2>/dev/null

    echo ""

    # Phase 2: ALL suites in parallel. Everything was pre-built in Phase 1,
    # so these invocations only RUN their binaries - no concurrent compile
    # into the shared target/, hence no build-artifact race.
    # - run_rust_workspace: --workspace --exclude ori_llvm --lib --bins --tests (run pre-built)
    # - run_rust_doctests: --workspace --doc (the LONE compiling invocation)
    # - run_rust_rt: -p ori_rt (run pre-built)
    # - run_rust_llvm: -p ori_llvm --lib (run pre-built)
    # - run_aot: -p ori_llvm --test aot (run pre-built)
    # - run_ori_interpreter: direct binary (./target/debug/ori), no cargo
    # - run_ori_llvm: direct binary (./target/release/ori), no cargo
    # - run_wasm_build: separate target dir (website playground workspace)
    # AOT identity-gate baseline (Go model: pin at start, immune thereafter).
    # The AOT harness snapshots the compiler binary + staticlib into a
    # per-process hardlink stage and records the staged source identities in
    # $AOT_STAGE_MANIFEST; the post-leg gate validates THAT manifest against
    # this baseline - live target/ churn after staging is harmless. Delete any
    # stale manifest first: post-leg presence proves THIS run staged.
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

    wait $RUST_PID || RUST_EXIT=1
    wait $DOCTEST_PID || DOCTEST_EXIT=1
    wait $WASM_PID || WASM_EXIT=1
    wait $RUST_RT_PID || RUST_RT_EXIT=1
    wait $RUST_LLVM_PID || RUST_LLVM_EXIT=1
    wait $AOT_PID || AOT_EXIT=1
    ORI_INTERP_EXIT=0
    wait $ORI_INTERP_PID || ORI_INTERP_EXIT=$?
    if [[ $LLVM_BUILD_OK -eq 1 ]]; then
        ORI_LLVM_EXIT=0
        wait $ORI_LLVM_PID || ORI_LLVM_EXIT=$?
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
    echo "=== Building LLVM release binary ==="
    LLVM_BUILD_OK=1
    if ! build_with_race_retry cargo build -p oric -p ori_rt --release -q; then
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
    # AOT identity-gate baseline + stale-manifest delete (see the parallel
    # block's comment for the Go-model contract).
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
