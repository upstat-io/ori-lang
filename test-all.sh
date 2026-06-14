#!/bin/bash
# Run ALL tests: Rust unit tests and Ori language tests
# Usage: ./test-all [-v|--verbose] [-s|--sequential] [--json[=<path>]]
#
# This script runs:
# 1. Rust unit tests (workspace default members — excludes ori_llvm)
# 2. Runtime library tests (ori_rt)
# 3. Rust unit tests (ori_llvm)
# 4. AOT integration tests (compile-and-run through ori build)
# 5. External playground WASM build (cargo build of ori-lang-website crate;
#    does NOT exercise Ori's own --target= path — those checks live in the
#    AOT integration suite under compiler/ori_llvm/tests/aot/{cli,cross}.rs)
# 6. Ori language spec tests (interpreter backend)
# 7. Ori language spec tests (LLVM backend)
#
# By default, runs tests in parallel for faster execution.
# Use -s or --sequential for sequential execution.
# Use -v or --verbose to see all output.

set -e

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

# Serialize whole runs: a second test-all queues instead of interleaving.
# Concurrent runs rebuild shared target/ artifacts mid-suite and mass-fail the
# AOT leg with bogus per-test failures. flock --close drops the lock fd before
# exec'ing children so cargo/ori subprocesses never inherit it; the kernel
# releases the lock if the holder dies.
mkdir -p target
if [ -z "${ORI_TESTALL_FLOCKED:-}" ] && command -v flock >/dev/null 2>&1; then
    if ! flock --nonblock target/.test-all.lock true 2>/dev/null; then
        echo "waiting for a concurrent test-all run to finish..."
    fi
    export ORI_TESTALL_FLOCKED=1
    exec flock --close target/.test-all.lock "$0" "$@"
fi

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

# Cleanup temp files on exit
cleanup() {
    rm -f "$RUST_OUTPUT" "$RUST_RT_OUTPUT" "$RUST_LLVM_OUTPUT" "$DOCTEST_OUTPUT" "$AOT_OUTPUT" "$WASM_OUTPUT" "$ORI_INTERP_OUTPUT" "$ORI_LLVM_OUTPUT"
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
# Walking-skeleton JSON-spine probe (throwaway scaffold; a later durable
# all-suites JSON-consumption path replaces it).
WALKING_SKELETON_JSON_EXIT=0

# --- Verification gates (enabled by default) ---
# ARC IR verification: checks RC balance, drop placement after AIMS pipeline.
# Also enables per-function LLVM IR verification at all emission sites.
export ORI_VERIFY_ARC=1
# LLVM pass verification: verifies IR well-formedness after every optimization pass.
# Measured overhead: ~20% wall time (54s vs ~45s baseline), within 150s budget.
export ORI_VERIFY_EACH=1

# --- Walking-skeleton JSON-spine probe (THROWAWAY SCAFFOLD) ---
# Proves the JSON spine end-to-end on ONE suite: the runner emits a structured
# {passed,failed,skipped} object behind --format=json (runner-emit), this
# harness consumes it via python3 json.load rather than a bash text scrape
# (harness-consume), asserts the parsed count equals the text-leg count
# (parity), and writes the object to a disc-backed artifact (record). A
# later durable all-suites JSON-consumption path retires this scaffold.
WALKING_SKELETON_JSON_SUITE="tests/spec/test_coalesce_copy.ori"
WALKING_SKELETON_JSON_RECORD="$(dirname "$0")/build/walking-skeleton-json-probe.json"
walking_skeleton_json_probe() {
    local bin="./target/debug/ori"
    [ -x "$bin" ] || { echo "walking-skeleton json probe: ori binary missing"; return 1; }
    [ -e "$WALKING_SKELETON_JSON_SUITE" ] || { echo "walking-skeleton json probe: suite $WALKING_SKELETON_JSON_SUITE missing"; return 1; }
    local json json_passed text_passed
    json=$("$bin" test --format=json "$WALKING_SKELETON_JSON_SUITE" 2>/dev/null)
    # Harness-consume leg: parse via python3 json.load (NOT a bash text scrape).
    json_passed=$(printf '%s' "$json" | python3 -c "import json,sys; print(json.load(sys.stdin)['passed'])" 2>/dev/null) \
        || { echo "walking-skeleton json probe: json.load failed on: $json"; return 1; }
    # Text leg: the legacy scrape, retained only for the parity comparison.
    text_passed=$("$bin" test "$WALKING_SKELETON_JSON_SUITE" 2>/dev/null \
        | grep -oE "[0-9]+ passed" | head -1 | grep -oE "[0-9]+")
    if [ "$json_passed" != "$text_passed" ]; then
        echo "walking-skeleton json probe: PARITY MISMATCH (json=$json_passed text=$text_passed)"
        return 1
    fi
    # Record leg: one number reaches a disc-backed artifact.
    mkdir -p "$(dirname "$WALKING_SKELETON_JSON_RECORD")"
    printf '%s\n' "$json" > "$WALKING_SKELETON_JSON_RECORD"
    echo "walking-skeleton json probe: OK (passed=$json_passed, parity with text leg, recorded to $WALKING_SKELETON_JSON_RECORD)"
    return 0
}

# --- Test runner functions ---

# Concurrent-build-race signature: a parallel `cargo`/`cargo clean` on the
# shared target/ corrupts in-flight build artifacts, so cargo cannot write a
# `.rmeta`/`.rlib`/`.o` it expects to exist. This is a transient INFRASTRUCTURE
# error, NOT a compile or test failure — reporting it as a suite "FAILED" is a
# spurious result. The signature is stable across cargo versions.
BUILD_RACE_RE='os error 2|could not write output|error: failed to write|No such file or directory \(os error 2\)'

# cargo_race_retry <output_file> <cargo args...>
# Runs a cargo invocation; on a build-artifact race (signature above) waits and
# retries ONCE (the racing build usually finishes in seconds). A genuine
# compile/test failure (no race signature) returns immediately — never masked.
cargo_race_retry() {
    local out="$1"; shift
    local attempt
    for attempt in 1 2; do
        if cargo "$@" > "$out" 2>&1; then
            return 0
        fi
        if [[ $attempt -eq 1 ]] && grep -qE "$BUILD_RACE_RE" "$out"; then
            echo "  ⚠ build-artifact race (os error 2) — concurrent cargo on shared target/; retrying once after 5s" >&2
            sleep 5
            continue
        fi
        return 1
    done
    return 1
}

run_rust_workspace() {
    echo "=== Running Rust unit tests (workspace) ==="
    # --lib --bins --tests excludes doctests: doctests compile under rustdoc at
    # RUN time (not prebuildable by `--no-run`), so they live in
    # run_rust_doctests as the lone compiling invocation of the parallel phase.
    if cargo_race_retry "$RUST_OUTPUT" test --workspace --exclude ori_llvm --lib --bins --tests; then
        echo "  ✓ Rust workspace tests passed"
        return 0
    else
        echo "  ✗ Rust workspace tests FAILED"
        return 1
    fi
}

run_rust_doctests() {
    echo "=== Running Rust doctests (workspace + ori_llvm) ==="
    # ONE rustdoc-compiling invocation for the whole workspace — the lone
    # compiler in the parallel phase (nothing concurrent compiles into
    # target/, so no build-artifact race). Own output file: run_rust_workspace
    # truncates RUST_OUTPUT concurrently, so appending there would clobber.
    if cargo test --workspace --doc > "$DOCTEST_OUTPUT" 2>&1; then
        echo "  ✓ Rust doctests passed"
        return 0
    else
        echo "  ✗ Rust doctests FAILED"
        return 1
    fi
}

run_rust_rt() {
    echo "=== Running runtime library tests (ori_rt) ==="
    if cargo_race_retry "$RUST_RT_OUTPUT" test -p ori_rt; then
        echo "  ✓ Runtime library tests passed"
        return 0
    else
        echo "  ✗ Runtime library tests FAILED"
        return 1
    fi
}

run_rust_llvm() {
    echo "=== Running Rust unit tests (ori_llvm) ==="
    # Lib binary is pre-built in Phase 1 (run-only here). ori_llvm doctests
    # run inside run_rust_doctests' single workspace --doc invocation.
    if cargo_race_retry "$RUST_LLVM_OUTPUT" test -p ori_llvm --lib; then
        echo "  ✓ Rust LLVM tests passed"
        return 0
    else
        echo "  ✗ Rust LLVM tests FAILED"
        return 1
    fi
}

run_aot() {
    echo "=== Running AOT integration tests ==="
    if cargo_race_retry "$AOT_OUTPUT" test -p ori_llvm --test aot; then
        echo "  ✓ AOT integration tests passed"
        return 0
    else
        echo "  ✗ AOT integration tests FAILED"
        return 1
    fi
}

run_wasm_build() {
    # WHAT THIS CHECKS:
    # This builds the EXTERNAL ori-lang-website playground crate against the
    # Rust `wasm32-unknown-unknown` target via `cargo build`. It does NOT
    # exercise Ori's own `ori build --target=...` code path, does NOT exercise
    # `TargetConfig::from_triple`, and does NOT exercise the `wasm32-wasi`
    # target. Coverage of Ori's WASM targets lives in:
    #   - `compiler/ori_llvm/tests/aot/cli.rs` — `ori build --target=...`
    #   - `compiler/ori_llvm/tests/aot/cross.rs` — `from_triple` round-trips
    # If you change Ori's WASM/WASI codegen or target parsing, those suites
    # are what cover you — not this check. See the regression history for the
    # category that this label could mask if misread.
    echo "=== Checking external playground WASM build ==="
    if ! rustup target list --installed | grep -q wasm32-unknown-unknown; then
        echo "  (skipped - wasm32-unknown-unknown rustc target not installed)"
        echo "skipped" > "$WASM_OUTPUT"
        return 0
    fi
    local wasm_manifest="../ori-lang-website/playground-wasm/Cargo.toml"
    if [ ! -f "$wasm_manifest" ]; then
        echo "  (skipped - ori-lang-website repo not found as sibling)"
        echo "skipped" > "$WASM_OUTPUT"
        return 0
    fi
    if cargo build --manifest-path "$wasm_manifest" --target wasm32-unknown-unknown --release > "$WASM_OUTPUT" 2>&1; then
        echo "  ✓ External playground WASM build passed"
        return 0
    else
        echo "  ✗ External playground WASM build FAILED"
        return 1
    fi
}

run_ori_interpreter() {
    echo "=== Running Ori language tests (interpreter) ==="
    # Use pre-built binary directly to avoid cargo lock contention.
    # target/debug/ori exists after workspace tests compile oric.
    if ./target/debug/ori test --verbose tests/ > "$ORI_INTERP_OUTPUT" 2>&1; then
        grep -E "[0-9]+ passed, [0-9]+ failed" "$ORI_INTERP_OUTPUT" | tail -1 | sed 's/^/  /'
        return 0
    else
        echo "  ✗ Ori interpreter tests FAILED"
        return 1
    fi
}

run_ori_llvm() {
    echo "=== Running Ori language tests (LLVM backend) ==="
    # Skip on Windows: JIT spec tests use setjmp/longjmp recovery which is not available
    # on MSVC (Windows uses SEH). AOT integration tests already cover LLVM codegen on Windows.
    case "$(uname -s)" in
        MINGW*|MSYS*|CYGWIN*|*NT*)
            echo "  (skipped on Windows — JIT recovery not supported; AOT tests cover LLVM codegen)"
            echo "skipped" > "$ORI_LLVM_OUTPUT"
            return 0
            ;;
    esac
    # Assumes LLVM release build (target/release/ori + libori_rt.a) was done in a prior phase.
    # Capture both stdout and stderr
    ./target/release/ori test --verbose --backend=llvm tests/ > "$ORI_LLVM_OUTPUT" 2>&1
    local exit_code=$?
    if [ $exit_code -eq 0 ]; then
        grep -E "[0-9]+ passed, [0-9]+ failed" "$ORI_LLVM_OUTPUT" | tail -1 | sed 's/^/  /'
        return 0
    elif [ $exit_code -gt 128 ]; then
        # Process was killed by signal (128 + signal number)
        local signal=$((exit_code - 128))
        # Show the actual error message
        local error_msg=$(grep -i "error\|panic" "$ORI_LLVM_OUTPUT" | head -1)
        if [ -n "$error_msg" ]; then
            echo "  ✗ Ori LLVM backend CRASHED: $error_msg"
        else
            echo "  ✗ Ori LLVM backend CRASHED (signal $signal)"
        fi
        return 1
    else
        echo "  ✗ Ori LLVM tests FAILED"
        return 1
    fi
}

# --- Parse test results functions ---

parse_rust_results() {
    local output_file=$1
    local prefix=$2

    local passed=$(grep -E "^test result:" "$output_file" 2>/dev/null | grep -oE '[0-9]+ passed' | awk '{sum += $1} END {print sum+0}')
    local failed=$(grep -E "^test result:" "$output_file" 2>/dev/null | sed 's/.*; \([0-9]*\) failed.*/\1/' | awk '{sum += $1} END {print sum+0}')
    local ignored=$(grep -E "^test result:" "$output_file" 2>/dev/null | sed 's/.*; \([0-9]*\) ignored.*/\1/' | awk '{sum += $1} END {print sum+0}')

    eval "${prefix}_PASSED=$passed"
    eval "${prefix}_FAILED=$failed"
    eval "${prefix}_IGNORED=$ignored"
}

parse_ori_results() {
    local output_file=$1
    local prefix=$2
    local exit_code=$3  # Pass exit code to detect crashes

    # Check for crash (signal-terminated process)
    if [ "${exit_code:-0}" -gt 128 ]; then
        eval "${prefix}_PASSED=0"
        eval "${prefix}_FAILED=0"
        eval "${prefix}_SKIPPED=0"
        eval "${prefix}_LCFAIL=0"
        eval "${prefix}_CRASHED=1"
        return
    fi

    local line=$(grep -E "[0-9]+ passed, [0-9]+ failed" "$output_file" 2>/dev/null | tail -1)
    local nums=($(echo "$line" | grep -oE '[0-9]+'))

    eval "${prefix}_PASSED=${nums[0]:-0}"
    eval "${prefix}_FAILED=${nums[1]:-0}"
    eval "${prefix}_SKIPPED=${nums[2]:-0}"
    eval "${prefix}_CRASHED=0"

    # Extract LLVM compile fail count (appears as "N llvm compile fail" in summary)
    # Use grep -o for macOS compatibility (no -P needed)
    local lcfail=$(echo "$line" | grep -o '[0-9]* llvm compile fail' | grep -o '[0-9]*')
    eval "${prefix}_LCFAIL=${lcfail:-0}"
}

# SSOT for a suite's status. Derives from exit code AND parsed counts so a
# suite that could not build/run (nonzero exit, no "test result:" line to
# parse) reports "errored" — NEVER a false "passed". The table, the JSON
# summary, and state.json all consume this one function.
#   failed > 0          -> failed
#   exit != 0, no fails  -> errored (build/run failure or pre-output crash)
#   otherwise            -> passed
# Identity fingerprint (dev:inode:mtime:size) of the shared AOT artifacts.
# GNU stat first; BSD/macOS fallback.
# KEEP IN SYNC with artifact_identity_of() in
# compiler/ori_llvm/tests/aot/util/binary.rs — the AOT harness writes the
# same tuple into its stage manifest; the gate below string-compares the two.
artifact_identity() {
    stat -c '%d:%i:%Y:%s' target/debug/ori target/debug/libori_rt.a 2>/dev/null \
        || stat -f '%d:%i:%m:%z' target/debug/ori target/debug/libori_rt.a 2>/dev/null \
        || echo "absent"
}

# Stage manifest the AOT harness publishes while snapshotting its per-process
# artifacts (staged_artifacts_dir in compiler/ori_llvm/tests/aot/util/binary.rs).
# Deleted at baseline capture, so post-leg presence proves THIS run staged.
AOT_STAGE_MANIFEST="build/aot-stage-manifest-debug.txt"

# Stage-time source identities recorded in the manifest for the same artifacts
# artifact_identity() stats, newline-joined in the same order. Empty output =
# manifest missing or unparseable (gate falls back to the live compare).
manifest_source_identity() {
    local manifest="$1" ori_id rt_id
    [ -f "$manifest" ] || return 0
    ori_id=$(awk '$1 == "artifact" && $2 == "ori" { print $4; exit }' "$manifest")
    rt_id=$(awk '$1 == "artifact" && $2 == "libori_rt.a" { print $4; exit }' "$manifest")
    if [ -n "$ori_id" ] && [ -n "$rt_id" ]; then
        printf '%s\n%s\n' "$ori_id" "$rt_id"
    fi
}

suite_status() {
    local exit_code="${1:-0}" failed="${2:-0}"
    if [ "$failed" -gt 0 ]; then
        echo "failed"
    elif [ "$exit_code" -ne 0 ]; then
        echo "errored"
    else
        echo "passed"
    fi
}

# --- Main execution ---

# Flag consistency check (blocking — abort on failure)
echo "=== Checking debug flag consistency ==="
if diagnostics/check-debug-flags.sh --no-color > /dev/null 2>&1; then
    echo "  ✓ Debug flag consistency check passed"
else
    echo -e "${RED}  ✗ Debug flag consistency check FAILED${NC}"
    echo "    Run 'diagnostics/check-debug-flags.sh' for details."
    exit 1
fi
echo ""

if [[ $PARALLEL -eq 1 ]]; then
    echo -e "${BOLD}Running tests in parallel...${NC}"
    echo ""

    # Phase 1: ONE serial pre-build covering every debug artifact the run
    # phase needs — workspace test binaries (incl. ori_llvm lib/aot/codegen),
    # the ori bin, and libori_rt.a. A single cargo invocation parallelizes its
    # own job graph internally (optimal), and serializing it ahead of the run
    # phase prevents the shared-target/ build-artifact race (`failed to write
    # ...rmeta: No such file (os error 2)`) that concurrent compiling cargo
    # invocations trigger. After this phase, every run invocation is pure
    # test execution (the lone exception is run_rust_doctests — rustdoc
    # compiles at run time and runs as the single compiler in Phase 2).
    echo "=== Pre-building all debug test artifacts (serial, race-free) ==="
    PRECOMPILE_OUTPUT=$(mktemp)
    if cargo_race_retry "$PRECOMPILE_OUTPUT" test --no-run -q --workspace --lib --bins --tests; then
        echo "  ✓ Debug test binaries pre-built"
    else
        echo -e "  ${RED}✗ Debug test-binary pre-build FAILED${NC}"
        cat "$PRECOMPILE_OUTPUT"
    fi
    rm -f "$PRECOMPILE_OUTPUT"

    # ori bin + libori_rt.a staticlib (cargo test --no-run builds the rlib,
    # not the staticlib; the interpreter suite + AOT links need these).
    if ! cargo build -p oric -p ori_rt -q 2>&1; then
        echo -e "  ${RED}✗ Debug ori/ori_rt build FAILED${NC}"
    fi

    # LLVM release build (sequential — shares target/ with the debug build).
    echo "=== Building LLVM release binary ==="
    LLVM_BUILD_OK=1
    if ! cargo build -p oric -p ori_rt --release -q 2>&1; then
        echo -e "  ${RED}✗ LLVM release build FAILED — skipping LLVM spec tests${NC}"
        LLVM_BUILD_OK=0
        ORI_LLVM_EXIT=1
    fi

    echo ""

    # Phase 2: ALL suites in parallel. Everything was pre-built in Phase 1,
    # so these invocations only RUN their binaries — no concurrent compile
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
    # this baseline — live target/ churn after staging is harmless. Delete any
    # stale manifest first: post-leg presence proves THIS run staged.
    rm -f "$AOT_STAGE_MANIFEST"
    AOT_ARTIFACT_BASELINE=$(artifact_identity)

    run_rust_workspace &
    RUST_PID=$!

    run_rust_doctests &
    DOCTEST_PID=$!

    run_wasm_build &
    WASM_PID=$!

    run_rust_rt &
    RUST_RT_PID=$!

    run_rust_llvm &
    RUST_LLVM_PID=$!

    run_aot &
    AOT_PID=$!

    run_ori_interpreter &
    ORI_INTERP_PID=$!

    if [[ $LLVM_BUILD_OK -eq 1 ]]; then
        run_ori_llvm &
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

    run_rust_workspace || RUST_EXIT=1
    echo ""
    echo "=== Building runtime library (debug) ==="
    if ! cargo build -p ori_rt -q 2>&1; then
        echo -e "  ${RED}✗ Runtime library debug build FAILED${NC}"
    fi
    echo "=== Building LLVM release binary ==="
    LLVM_BUILD_OK=1
    if ! cargo build -p oric -p ori_rt --release -q 2>&1; then
        echo -e "  ${RED}✗ LLVM release build FAILED — skipping LLVM spec tests${NC}"
        LLVM_BUILD_OK=0
        ORI_LLVM_EXIT=1
    fi
    echo ""
    run_rust_rt || RUST_RT_EXIT=1
    echo ""
    run_rust_llvm || RUST_LLVM_EXIT=1
    echo ""
    run_rust_doctests || DOCTEST_EXIT=1
    echo ""
    # AOT identity-gate baseline + stale-manifest delete (see the parallel
    # block's comment for the Go-model contract).
    rm -f "$AOT_STAGE_MANIFEST"
    AOT_ARTIFACT_BASELINE=$(artifact_identity)
    run_aot || AOT_EXIT=1
    echo ""
    run_wasm_build || WASM_EXIT=1
    echo ""
    ORI_INTERP_EXIT=0
    run_ori_interpreter || ORI_INTERP_EXIT=$?
    echo ""
    if [[ $LLVM_BUILD_OK -eq 1 ]]; then
        ORI_LLVM_EXIT=0
        run_ori_llvm || ORI_LLVM_EXIT=$?
    fi
fi

# Show verbose output if requested or on failure
# AOT identity gate (Go model) — decide BEFORE any failure dump: invalidated
# per-test output is noise, not signal. Decision table:
#   manifest present, staged identities == baseline -> VALID (the per-process
#     snapshot pinned the baseline inodes; later target/ churn never reached
#     the leg — no invalidation even if artifact_identity changed mid-run)
#   manifest present, staged identities != baseline -> INVALID (a build
#     crossed the baseline->stage window; the AOT leg ran different artifacts
#     than the rest of the run)
#   manifest absent/unparseable -> conservative legacy live compare
AOT_INVALID=0
if [ -n "${AOT_ARTIFACT_BASELINE:-}" ]; then
    AOT_STAGED_IDENTITY=$(manifest_source_identity "$AOT_STAGE_MANIFEST")
    if [ -n "$AOT_STAGED_IDENTITY" ]; then
        if [ "$AOT_STAGED_IDENTITY" != "$AOT_ARTIFACT_BASELINE" ]; then
            AOT_INVALID=1
            echo ""
            echo -e "${RED}AOT LEG INVALID - the AOT harness staged artifacts differing from the pre-run baseline (a build replaced them before staging); AOT counts are not trustworthy${NC}"
        fi
    elif [ "$(artifact_identity)" != "$AOT_ARTIFACT_BASELINE" ]; then
        AOT_INVALID=1
        echo ""
        echo -e "${RED}AOT LEG INVALID - build artifacts changed mid-run and no stage manifest was published (snapshot staging never ran); AOT counts are not trustworthy${NC}"
    fi
fi

if [[ $VERBOSE -eq 1 ]]; then
    echo ""
    echo "=== Detailed Output ==="
    echo ""
    echo "--- Rust workspace tests ---"
    cat "$RUST_OUTPUT"
    echo ""
    echo "--- Runtime library tests ---"
    cat "$RUST_RT_OUTPUT"
    echo ""
    echo "--- Rust LLVM tests ---"
    cat "$RUST_LLVM_OUTPUT"
    echo ""
    echo "--- AOT integration tests ---"
    cat "$AOT_OUTPUT"
    echo ""
    echo "--- WASM build ---"
    cat "$WASM_OUTPUT"
    echo ""
    echo "--- Ori interpreter tests ---"
    cat "$ORI_INTERP_OUTPUT"
    echo ""
    echo "--- Ori LLVM tests ---"
    cat "$ORI_LLVM_OUTPUT"
else
    # Show output only for failed tests
    if [[ $RUST_EXIT -ne 0 ]]; then
        echo ""
        echo -e "${RED}--- Rust workspace test failures ---${NC}"
        cat "$RUST_OUTPUT"
    fi
    if [[ $RUST_RT_EXIT -ne 0 ]]; then
        echo ""
        echo -e "${RED}--- Runtime library test failures ---${NC}"
        cat "$RUST_RT_OUTPUT"
    fi
    if [[ $RUST_LLVM_EXIT -ne 0 ]]; then
        echo ""
        echo -e "${RED}--- Rust LLVM test failures ---${NC}"
        cat "$RUST_LLVM_OUTPUT"
    fi
    if [[ $AOT_EXIT -ne 0 && $AOT_INVALID -ne 1 ]]; then
        echo ""
        echo -e "${RED}--- AOT integration test failures ---${NC}"
        cat "$AOT_OUTPUT"
    fi
    if [[ $WASM_EXIT -ne 0 ]]; then
        echo ""
        echo -e "${RED}--- WASM build failures ---${NC}"
        cat "$WASM_OUTPUT"
    fi
    if [[ $ORI_INTERP_EXIT -ne 0 ]]; then
        echo ""
        echo -e "${RED}--- Ori interpreter test failures ---${NC}"
        cat "$ORI_INTERP_OUTPUT"
    fi
    if [[ $ORI_LLVM_EXIT -ne 0 ]]; then
        echo ""
        echo -e "${RED}--- Ori LLVM test failures ---${NC}"
        cat "$ORI_LLVM_OUTPUT"
    fi
fi

# Parse all results
parse_rust_results "$RUST_OUTPUT" "RUST"
parse_rust_results "$RUST_RT_OUTPUT" "RUST_RT"
parse_rust_results "$RUST_LLVM_OUTPUT" "RUST_LLVM"
parse_rust_results "$AOT_OUTPUT" "AOT"
parse_rust_results "$DOCTEST_OUTPUT" "DOCTEST"
parse_ori_results "$ORI_INTERP_OUTPUT" "ORI_INTERP" "$ORI_INTERP_EXIT"
parse_ori_results "$ORI_LLVM_OUTPUT" "ORI_LLVM" "$ORI_LLVM_EXIT"

# Walking-skeleton JSON-spine probe: runs after the debug binary is built
# (the interpreter suite above relies on ./target/debug/ori). A parity
# mismatch or a json.load failure fails the run via ANY_CORE_FAILED below.
walking_skeleton_json_probe || WALKING_SKELETON_JSON_EXIT=$?

# Count AOT tests that failed specifically due to memory leaks.
# assert_aot_success panics with "leaked memory" when exit code is 2.
AOT_LEAKS=$(grep -c "leaked memory" "$AOT_OUTPUT" 2>/dev/null || true)
AOT_LEAKS=${AOT_LEAKS:-0}

# Artifact-identity invalidation (detected above, pre-dump): zero the counts
# and force the errored pathway so the table + TOTAL report INCOMPLETE instead
# of bogus per-test failures.
if [ "${AOT_INVALID:-0}" = "1" ]; then
    AOT_FAILED=0
    AOT_PASSED=0
    AOT_IGNORED=0
    AOT_EXIT=1
fi

# Determine WASM status
if grep -q "skipped" "$WASM_OUTPUT" 2>/dev/null; then
    WASM_STATUS="skipped"
elif [[ $WASM_EXIT -eq 0 ]]; then
    WASM_STATUS="passed"
else
    WASM_STATUS="FAILED"
fi

# Per-suite status (SSOT via suite_status) for the Rust suites. An "errored"
# suite built/ran with a nonzero exit but produced no parseable results — it is
# surfaced loudly below and counted as a failure, never a silent green.
RUST_STATUS=$(suite_status "$RUST_EXIT" "$RUST_FAILED")
DOCTEST_STATUS=$(suite_status "$DOCTEST_EXIT" "$DOCTEST_FAILED")
RUST_RT_STATUS=$(suite_status "$RUST_RT_EXIT" "$RUST_RT_FAILED")
RUST_LLVM_STATUS=$(suite_status "$RUST_LLVM_EXIT" "$RUST_LLVM_FAILED")
AOT_STATUS=$(suite_status "$AOT_EXIT" "$AOT_FAILED")
ORI_INTERP_STATUS=$(suite_status "$ORI_INTERP_EXIT" "$ORI_INTERP_FAILED")

ERRORED_SUITES=""
INCOMPLETE_SUITES=0
for pair in \
    "Rust unit tests (workspace)=$RUST_STATUS" \
    "Rust doctests (workspace)=$DOCTEST_STATUS" \
    "Runtime library (ori_rt)=$RUST_RT_STATUS" \
    "Rust unit tests (ori_llvm)=$RUST_LLVM_STATUS" \
    "AOT integration tests=$AOT_STATUS" \
    "Ori spec (interpreter)=$ORI_INTERP_STATUS"; do
    if [ "${pair##*=}" = "errored" ]; then
        ERRORED_SUITES="${ERRORED_SUITES}  - ${pair%=*}\n"
        INCOMPLETE_SUITES=$((INCOMPLETE_SUITES + 1))
    fi
done
# LLVM spec backend that never produced counts (build failure or crash) also
# leaves the TOTAL row incomplete.
if [ "${LLVM_BUILD_OK:-1}" -eq 0 ] || [ "${ORI_LLVM_CRASHED:-0}" -eq 1 ]; then
    INCOMPLETE_SUITES=$((INCOMPLETE_SUITES + 1))
fi

# --- Print Summary ---
echo ""
echo "=============================================="
echo -e "${BOLD}                TEST SUMMARY${NC}"
echo "=============================================="
echo ""
# Render one Rust suite row. An "errored" suite (built/ran with no parseable
# results) prints ERR markers in red instead of a misleading 0/0/0.
print_rust_row() {
    local name="$1" passed="$2" failed="$3" skipped="$4" status="$5" extra="${6:-}"
    if [ "$status" = "errored" ]; then
        printf "%-30s %8s %8s %8s %8s  ${RED}<- build/run failed (no results)${NC}\n" \
            "$name" "ERR" "ERR" "ERR" "-"
    else
        printf "%-30s %8d %8d %8d %8s%s\n" "$name" "$passed" "$failed" "$skipped" "-" "$extra"
    fi
}

printf "%-30s %8s %8s %8s %8s\n" "Test Suite" "Passed" "Failed" "Skipped" "LCFail"
printf "%-30s %8s %8s %8s %8s\n" "------------------------------" "--------" "--------" "--------" "--------"
print_rust_row "Rust unit tests (workspace)" "$RUST_PASSED" "$RUST_FAILED" "$RUST_IGNORED" "$RUST_STATUS"
print_rust_row "Runtime library (ori_rt)" "$RUST_RT_PASSED" "$RUST_RT_FAILED" "$RUST_RT_IGNORED" "$RUST_RT_STATUS"
print_rust_row "Rust unit tests (ori_llvm)" "$RUST_LLVM_PASSED" "$RUST_LLVM_FAILED" "$RUST_LLVM_IGNORED" "$RUST_LLVM_STATUS"
if [ "$AOT_LEAKS" -gt 0 ] && [ "$AOT_STATUS" != "errored" ]; then
    print_rust_row "AOT integration tests" "$AOT_PASSED" "$AOT_FAILED" "$AOT_IGNORED" "$AOT_STATUS" "$(printf '  %b(%d leaked)%b' "$YELLOW" "$AOT_LEAKS" "$NC")"
else
    print_rust_row "AOT integration tests" "$AOT_PASSED" "$AOT_FAILED" "$AOT_IGNORED" "$AOT_STATUS"
fi
print_rust_row "Rust doctests (workspace)" "$DOCTEST_PASSED" "$DOCTEST_FAILED" "$DOCTEST_IGNORED" "$DOCTEST_STATUS"
printf "%-30s %8s\n" "External playground WASM" "$WASM_STATUS"
print_rust_row "Ori spec (interpreter)" "$ORI_INTERP_PASSED" "$ORI_INTERP_FAILED" "$ORI_INTERP_SKIPPED" "$ORI_INTERP_STATUS"
if grep -qx "skipped" "$ORI_LLVM_OUTPUT" 2>/dev/null; then
    printf "%-30s %8s\n" "Ori spec (LLVM backend)" "skipped"
elif [ "${LLVM_BUILD_OK:-1}" -eq 0 ]; then
    printf "%-30s %8s\n" "Ori spec (LLVM backend)" "BUILD FAILED"
elif [ "${ORI_LLVM_CRASHED:-0}" -eq 1 ]; then
    printf "%-30s %8s\n" "Ori spec (LLVM backend)" "CRASHED"
else
    printf "%-30s %8d %8d %8d %8d\n" "Ori spec (LLVM backend)" "$ORI_LLVM_PASSED" "$ORI_LLVM_FAILED" "$ORI_LLVM_SKIPPED" "${ORI_LLVM_LCFAIL:-0}"
fi
printf "%-30s %8s %8s %8s %8s\n" "------------------------------" "--------" "--------" "--------" "--------"

# Calculate totals
TOTAL_PASSED=$((DOCTEST_PASSED + RUST_PASSED + RUST_RT_PASSED + RUST_LLVM_PASSED + AOT_PASSED + ORI_INTERP_PASSED + ORI_LLVM_PASSED))
TOTAL_FAILED=$((DOCTEST_FAILED + RUST_FAILED + RUST_RT_FAILED + RUST_LLVM_FAILED + AOT_FAILED + ORI_INTERP_FAILED + ORI_LLVM_FAILED))
TOTAL_SKIPPED=$((DOCTEST_IGNORED + RUST_IGNORED + RUST_RT_IGNORED + RUST_LLVM_IGNORED + AOT_IGNORED + ORI_INTERP_SKIPPED + ORI_LLVM_SKIPPED))
TOTAL_LCFAIL=$((${ORI_LLVM_LCFAIL:-0}))

printf "${BOLD}%-30s %8d %8d %8d %8d${NC}\n" "TOTAL" "$TOTAL_PASSED" "$TOTAL_FAILED" "$TOTAL_SKIPPED" "$TOTAL_LCFAIL"
if [ "$INCOMPLETE_SUITES" -gt 0 ]; then
    echo -e "${RED}${BOLD}TOTAL IS INCOMPLETE: $INCOMPLETE_SUITES suite(s) errored before producing counts — the real failure count is higher.${NC}"
fi
echo ""

# State self-update is invoked AFTER emit_json (defined ~line 611), so it runs
# below the JSON emit blocks at the file's tail. This block is intentionally
# empty here; see the matching block after the emit_json invocations.

if [ "$AOT_LEAKS" -gt 0 ]; then
    echo -e "${YELLOW}${BOLD}⚠  $AOT_LEAKS AOT test(s) leaked memory (ORI_CHECK_LEAKS=1 detected RC leaks)${NC}"
    echo ""
fi

# --- Diagnostic hints on LLVM/AOT failure (suppressed in JSON mode) ---
if [[ $EMIT_JSON -eq 0 ]] && [[ $RUST_LLVM_EXIT -ne 0 || $AOT_EXIT -ne 0 || $ORI_LLVM_EXIT -ne 0 ]]; then
    echo "  Diagnostic hints:"
    echo "    diagnose-aot.sh <file.ori>      — all-in-one AOT diagnostic"
    echo "    dual-exec-debug.sh <file.ori>   — compare interpreter vs AOT"
    echo "    bisect-passes.sh <file.ori>     — identify failing AIMS phase"
    echo "    codegen-audit.sh <file.ori>     — static RC/COW/ABI check"
    echo ""
fi

# --- Emit JSON if requested ---
# JSON-escape a string for a summary failure object. Strip the rare invisible
# C0 + DEL controls (no diagnostic value), then escape backslash, double-quote,
# and the meaningful controls tab/newline/CR per RFC 8259 §7. Uses bash
# parameter expansion: a bash variable holds embedded tab/newline/CR directly,
# so the escaping has no awk-FS / awk-stdin or line-oriented-sed pitfall (a
# naive `sed s/\n/.../` cannot match a newline in pattern space; awk in a pipe
# inside a `while read` loop mis-handles the record). Order is load-bearing:
# backslash escapes FIRST so a later "->\" does not double-escape the inserted
# backslash. SSOT for both parse_*_failures.
json_escape_string() {
    local s
    # Strip the rare-C0/DEL range (keep tab 0x09 / LF 0x0a / CR 0x0d for escaping).
    s=$(printf '%s' "$1" | tr -d '\000-\010\013\014\016-\037\177')
    s="${s//\\/\\\\}"      # backslash -> \\  (FIRST)
    s="${s//\"/\\\"}"      # double-quote -> \"
    s="${s//$'\t'/\\t}"    # tab -> \t
    s="${s//$'\n'/\\n}"    # newline -> \n
    s="${s//$'\r'/\\r}"    # CR -> \r
    printf '%s' "$s"
}

# Parse individual Rust test failures from a cargo test output log.
# Emits JSON array of failure objects on stdout.
parse_rust_failures() {
    local output_file="$1"
    local suite_id="$2"
    local failure_kind="$3"  # default failure kind
    [ ! -f "$output_file" ] && { echo '[]'; return; }
    local entries=""
    while IFS= read -r line; do
        # Match: "test <name> ... FAILED"
        if [[ "$line" =~ ^test[[:space:]](.*)[[:space:]]\.\.\.[[:space:]]FAILED$ ]]; then
            local test_name="${BASH_REMATCH[1]}"
            local escaped_test_name
            escaped_test_name=$(json_escape_string "$test_name")
            local kind="$failure_kind"
            # Try to classify more precisely from surrounding output
            if grep -q "panicked at" "$output_file" 2>/dev/null; then kind="panic"; fi
            local escaped_kind
            escaped_kind=$(printf '%s' "$kind")
            entries+="    { \"test_id\": \"$escaped_test_name\", \"test_id_kind\": \"rust\", \"suite\": \"$suite_id\", \"failure_kind\": \"$escaped_kind\", \"error_message\": \"test $escaped_test_name ... FAILED\" }"$'\n'
        fi
    done < "$output_file"
    # Emit as JSON array
    echo "$entries" | grep -v '^$' | sed '$!s/$/,/' | (echo '['; cat; echo '  ]') 2>/dev/null || echo '[]'
}

# Parse Ori test failures from an ori test runner output log.
# Emits JSON array of failure objects on stdout.
parse_ori_failures() {
    local output_file="$1"
    local suite_id="$2"
    [ ! -f "$output_file" ] && { echo '[]'; return; }
    local entries=""
    while IFS= read -r line; do
        # Match: lines containing "FAIL" (Ori test runner output)
        if [[ "$line" =~ FAIL ]]; then
            local test_name
            test_name=$(echo "$line" | sed 's/.*\[FAIL\] //' | sed 's/FAILED //' | tr -d '\n\r')
            local escaped_test_name escaped_error_msg
            escaped_test_name=$(json_escape_string "$test_name")
            escaped_error_msg=$(json_escape_string "$line")
            entries+="    { \"test_id\": \"$escaped_test_name\", \"test_id_kind\": \"ori_spec\", \"suite\": \"$suite_id\", \"failure_kind\": \"assertion_failure\", \"error_message\": \"$escaped_error_msg\" }"$'\n'
        fi
    done < "$output_file"
    echo "$entries" | grep -v '^$' | sed '$!s/$/,/' | (echo '['; cat; echo '  ]') 2>/dev/null || echo '[]'
}

emit_json() {
    local path="$1"
    local overall="passed"
    if [ "$ANY_FAILED" -ne 0 ]; then
        overall="failed"
    fi

    # Parse individual failures from each suite log
    local rust_failures ori_interp_failures ori_llvm_failures rt_failures rust_llvm_failures aot_failures
    rust_failures=$(parse_rust_failures "$RUST_OUTPUT" "rust_workspace" "panic")
    rt_failures=$(parse_rust_failures "$RUST_RT_OUTPUT" "rust_rt" "panic")
    rust_llvm_failures=$(parse_rust_failures "$RUST_LLVM_OUTPUT" "rust_llvm" "panic")
    local doctest_failures
    doctest_failures=$(parse_rust_failures "$DOCTEST_OUTPUT" "rust_doctest" "panic")
    aot_failures=$(parse_rust_failures "$AOT_OUTPUT" "aot" "panic")
    ori_interp_failures=$(parse_ori_failures "$ORI_INTERP_OUTPUT" "ori_interp")
    ori_llvm_failures=$(parse_ori_failures "$ORI_LLVM_OUTPUT" "ori_llvm")

    # Combine all failures
    # Build combined failures array by stripping outer brackets and concatenating
    local all_failures=""
    for failures in "$rust_failures" "$rt_failures" "$rust_llvm_failures" "$aot_failures" "$ori_interp_failures" "$ori_llvm_failures"; do
        local inner
        inner=$(echo "$failures" | sed '1d;$d' | grep -v '^$' || true)
        if [ -n "$inner" ]; then
            if [ -n "$all_failures" ]; then
                all_failures+=",$inner"$'\n'
            else
                all_failures="$inner"$'\n'
            fi
        fi
    done
    all_failures=${all_failures%,$'\n'}

    # Helper: emit a suite entry for per_suite (includes stable id + display_name).
    # $8 is the suite exit code: a nonzero exit with no parsed failures means the
    # suite could not build/run and is reported "errored", never a false "passed".
    json_suite_full() {
        local id="$1" display="$2" passed="$3" failed="$4" skipped="$5"
        local lcfail="${6:-0}" status="${7:-passed}" exit_code="${8:-0}"
        if [ "$failed" -gt 0 ]; then
            status="failed"
        elif [ "$exit_code" -ne 0 ] && [ "$status" = "passed" ]; then
            status="errored"
        fi
        printf '    "%s": { "display_name": "%s", "passed": %d, "failed": %d, "skipped": %d, "lcfail": %d, "status": "%s", "failed_attributed": 0, "failed_unattributed": %d }' \
            "$id" "$display" "${passed:-0}" "${failed:-0}" "${skipped:-0}" "${lcfail:-0}" "$status" "${failed:-0}"
    }

    local wasm_failed=0 wasm_passed=0
    if [ "$WASM_STATUS" = "passed" ]; then wasm_passed=1; else wasm_failed=1; fi

    local llvm_passed=0 llvm_failed=0 llvm_skipped=0 llvm_lcfail=0 llvm_status="passed"
    if [ "${LLVM_BUILD_OK:-1}" -eq 0 ]; then llvm_status="build_failed"
    elif [ "${ORI_LLVM_CRASHED:-0}" -eq 1 ]; then llvm_status="crashed"
    else
        llvm_passed=${ORI_LLVM_PASSED:-0}; llvm_failed=${ORI_LLVM_FAILED:-0}
        llvm_skipped=${ORI_LLVM_SKIPPED:-0}; llvm_lcfail=${ORI_LLVM_LCFAIL:-0}
        [ "$llvm_failed" -gt 0 ] && llvm_status="failed"
    fi

    {
        echo "{"
        echo "  \"timestamp\": \"$(date -u +%Y-%m-%dT%H:%M:%SZ)\","
        echo "  \"overall\": \"$overall\","
        echo "  \"failures\": ["
        if [ -n "$all_failures" ]; then
            echo "$all_failures"
        fi
        echo "  ],"
        echo "  \"per_suite\": {"
        json_suite_full "rust_workspace" "Rust unit tests (workspace)" "$RUST_PASSED" "$RUST_FAILED" "$RUST_IGNORED" 0 "passed" "$RUST_EXIT"
        echo ","
        json_suite_full "rust_rt" "Runtime library (ori_rt)" "$RUST_RT_PASSED" "$RUST_RT_FAILED" "$RUST_RT_IGNORED" 0 "passed" "$RUST_RT_EXIT"
        echo ","
        json_suite_full "rust_llvm" "Rust unit tests (ori_llvm)" "$RUST_LLVM_PASSED" "$RUST_LLVM_FAILED" "$RUST_LLVM_IGNORED" 0 "passed" "$RUST_LLVM_EXIT"
        echo ","
        json_suite_full "aot" "AOT integration tests" "$AOT_PASSED" "$AOT_FAILED" "$AOT_IGNORED" 0 "passed" "$AOT_EXIT"
        echo ","
        json_suite_full "rust_doctest" "Rust doctests (workspace)" "$DOCTEST_PASSED" "$DOCTEST_FAILED" "$DOCTEST_IGNORED" 0 "passed" "$DOCTEST_EXIT"
        echo ","
        printf '    "wasm_playground": { "display_name": "External playground WASM", "passed": %d, "failed": %d, "skipped": 0, "lcfail": 0, "status": "%s", "failed_attributed": 0, "failed_unattributed": %d }' "$wasm_passed" "$wasm_failed" "$WASM_STATUS" "$wasm_failed"
        echo ","
        json_suite_full "ori_interp" "Ori spec (interpreter)" "$ORI_INTERP_PASSED" "$ORI_INTERP_FAILED" "$ORI_INTERP_SKIPPED" 0 "passed" "$ORI_INTERP_EXIT"
        echo ","
        printf '    "ori_llvm": { "display_name": "Ori spec (LLVM backend)", "passed": %d, "failed": %d, "skipped": %d, "lcfail": %d, "status": "%s", "failed_attributed": 0, "failed_unattributed": %d }' "$llvm_passed" "$llvm_failed" "$llvm_skipped" "$llvm_lcfail" "$llvm_status" "$llvm_failed"
        echo ""
        echo "  },"
        echo "  \"totals\": { \"passed\": $TOTAL_PASSED, \"failed\": $TOTAL_FAILED, \"skipped\": $TOTAL_SKIPPED, \"lcfail\": $TOTAL_LCFAIL, \"aot_leaks\": $AOT_LEAKS }"
        echo "}"
    } > "$path"

    echo "Test results written to $path"
}


# Final status
# ONLY the ORI_LLVM_EXIT spec-backend SIGSEGV (ORI_LLVM_CRASHED, a known
# tracked crash) is exempted from the red verdict — and only when every other
# suite is clean. Every other suite, INCLUDING one that errored (built/ran with
# no parseable results), counts toward ANY_CORE_FAILED and fails the run. A
# build failure or crash is never a silent green; the divergence where local
# read green while CI read red came from suites reporting 0/0 "passed".
ANY_CORE_FAILED=$((RUST_EXIT + DOCTEST_EXIT + RUST_RT_EXIT + RUST_LLVM_EXIT + AOT_EXIT + WASM_EXIT + ORI_INTERP_EXIT + WALKING_SKELETON_JSON_EXIT))
ANY_FAILED=$((ANY_CORE_FAILED + ORI_LLVM_EXIT))

if [ -n "$ERRORED_SUITES" ]; then
    echo -e "${RED}${BOLD}Errored suites — built/ran with no parseable results (counted as FAILED):${NC}"
    echo -e "$ERRORED_SUITES"
    echo "  A suite errors when it cannot build or crashes before printing results."
    echo "  This is the case CI surfaces as failures; it is no longer hidden as 0/0 passed."
    echo ""
fi

if [[ $EMIT_JSON -eq 1 ]]; then
    emit_json "$JSON_PATH"
fi

# State self-update (test_suite + dispositions). Producer-writes-cache:
# test-all.sh just ran every suite; it owns the data, so it writes its own
# slice of state.json instead of relying on `state.sh refresh --full` to
# re-orchestrate the run. Always produces a summary to a default path when
# --json-summary wasn't specified, then ingests via state.sh refresh
# --from-summary. Suppressed in --json mode (machine consumers read state
# directly). Skipped when state.sh / jq is unavailable.
if [[ $EMIT_JSON -eq 0 ]] \
   && command -v jq >/dev/null 2>&1 \
   && [[ -x "$(dirname "$0")/diagnostics/state.sh" ]]; then
    if [[ -z "$JSON_SUMMARY_PATH" ]]; then
        JSON_SUMMARY_PATH="$(dirname "$0")/build/test-all-summary.json"
        EMIT_JSON_SUMMARY=1
    fi
    mkdir -p "$(dirname "$JSON_SUMMARY_PATH")"
    emit_json "$JSON_SUMMARY_PATH"
    INGEST_JSON=$("$(dirname "$0")/diagnostics/state.sh" refresh --from-summary="$JSON_SUMMARY_PATH" --json --by test-all 2>/dev/null || true)
    if [[ -n "$INGEST_JSON" ]]; then
        DISP_TOTAL=$(printf '%s' "$INGEST_JSON" | jq -r '.dispositions_total // 0')
        DISP_UNTRACKED=$(printf '%s' "$INGEST_JSON" | jq -r '.dispositions_untracked // 0')
        if [[ "$DISP_UNTRACKED" -gt 0 ]]; then
            echo -e "${RED}${BOLD}Dispositions: $DISP_TOTAL total, $DISP_UNTRACKED UNTRACKED — DRIFT${NC}"
            echo "  Every #[ignore]/#skip needs a tracking-bug ID in its reason text."
            echo "  List the offenders:"
            echo "    diagnostics/state.sh dispositions --untracked-only"
            echo ""
        else
            echo "Dispositions: $DISP_TOTAL total, 0 untracked"
            echo ""
        fi
    fi
elif [[ $EMIT_JSON_SUMMARY -eq 1 ]]; then
    # --json-summary was requested but ingest path is unavailable (no jq /
    # no state.sh). Honor the request anyway.
    emit_json "$JSON_SUMMARY_PATH"
fi

if [ "$ANY_FAILED" -eq 0 ]; then
    echo -e "${GREEN}${BOLD}=== All tests passed ===${NC}"
    exit 0
elif [ "$ANY_CORE_FAILED" -eq 0 ] && [ "${ORI_LLVM_CRASHED:-0}" -eq 1 ]; then
    echo -e "${YELLOW:-\033[0;33m}${BOLD}=== All other suites passed — LLVM backend spec tests CRASHED (known issue, not fixed) ===${NC}"
    echo -e "${YELLOW:-\033[0;33m}    This is the ONE tracked exemption. Every other suite, including errored ones, fails the run.${NC}"
    exit 0
else
    echo -e "${RED}${BOLD}=== Some tests failed ===${NC}"
    exit 1
fi
