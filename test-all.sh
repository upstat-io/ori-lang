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
# other's artifacts mid-suite and mass-fail the AOT leg with bogus failures, and
# a run sharing target/ with any other cargo (parallel sessions, reviewers)
# stalls on cargo's build lock while holding the whole-run lock — hanging every
# other run behind it. Each run builds in its OWN target/<build-id> so it never
# contends on a foreign cargo lock; sccache (below) shares the compile cache
# across these dirs so isolation is not a cold rebuild. The build id defaults to
# the session id when present (distinct sessions run fully concurrently), else
# "shared". Same-id runs still serialize on their own per-id lock (prevents the
# shared-artifact AOT corruption). flock --close drops the lock fd before
# exec'ing children so subprocesses never inherit it; the kernel releases it if
# the holder dies.
TESTALL_BUILD_ID="${ORI_TESTALL_BUILD_ID:-${CLAUDE_CODE_SESSION_ID:-shared}}"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$(pwd)/target/test-all-${TESTALL_BUILD_ID}}"
TARGET_DIR="$CARGO_TARGET_DIR"
mkdir -p "$TARGET_DIR" target
TESTALL_LOCK="target/.test-all-${TESTALL_BUILD_ID}.lock"
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
# $LEG_TIMING_DIR/<name>. Additive observability only — no pass/fail effect.
LEG_TIMING_DIR=$(mktemp -d)
# Machine-readable runner JSON (`ori test --format json`) per backend, written
# to disc-backed files (single run per backend; the console summary + failure
# detail are reconstructed from these via diagnostics/parse_test_json.py — no
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

# timed_leg <name> <command...>: run the command, record its wall-clock seconds
# to $LEG_TIMING_DIR/<name>, and pass through its exit code unchanged. Purely
# additive — the timing capture never alters the leg's behavior or result.
timed_leg() {
    local _name="$1"; shift
    local _t0 _t1 _rc
    _t0=$(date +%s.%N)
    "$@"
    _rc=$?
    _t1=$(date +%s.%N)
    awk "BEGIN { printf \"%.1f\", ${_t1} - ${_t0} }" > "$LEG_TIMING_DIR/${_name}" 2>/dev/null
    return $_rc
}

# Track failures
RUST_EXIT=0
DOCTEST_EXIT=0
RUST_RT_EXIT=0
RUST_LLVM_EXIT=0
AOT_EXIT=0
WASM_EXIT=0
ORI_INTERP_EXIT=0
ORI_LLVM_EXIT=0

# --- Verification gates (enabled by default) ---
# ARC IR verification: checks RC balance, drop placement after AIMS pipeline.
# Also enables per-function LLVM IR verification at all emission sites.
export ORI_VERIFY_ARC=1
# LLVM pass verification: verifies IR well-formedness after every optimization pass.
# Measured overhead: ~20% wall time (54s vs ~45s baseline), within 150s budget.
export ORI_VERIFY_EACH=1

# --- Test runner functions ---

# Concurrent-build-race signature: a parallel `cargo`/`cargo clean` on the
# shared target/ corrupts in-flight build artifacts, so cargo cannot write a
# `.rmeta`/`.rlib`/`.o` it expects to exist. This is a transient INFRASTRUCTURE
# error, NOT a compile or test failure — reporting it as a suite "FAILED" is a
# spurious result. The signature is stable across cargo versions.
BUILD_RACE_RE='os error 2|could not write output|error: failed to write|No such file or directory \(os error 2\)'

# cargo_race_retry <output_file> <runner-command...>
# Runs an arbitrary runner command (e.g. `cargo test ...` OR `cargo nextest run
# ...`); on a build-artifact race (signature above) waits and retries ONCE (the
# racing build usually finishes in seconds). A genuine compile/test failure (no
# race signature) returns immediately — never masked. Callers pass the FULL
# command including `cargo` so both the cargo-test and nextest legs share one
# race-retry surface.
cargo_race_retry() {
    local out="$1"; shift
    local attempt
    for attempt in 1 2; do
        if "$@" > "$out" 2>&1; then
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

# Run one Rust test leg: cargo-nextest when active (binaries pre-built --no-run
# in Phase 1, so this plain run does not recompile; cargo -p/--lib/--test
# selectors are accepted directly), else the cargo_race_retry cargo-test
# fallback. $1 = output file; remaining args = the shared leg selector. Output
# captured to $1 for parse_rust_results (nextest + cargo branches per format).
rust_test_leg() {
    local out="$1"; shift
    if [ -n "$NEXTEST_ACTIVE" ]; then
        # --no-fail-fast: full-suite run (no early abort), matching the cargo path.
        # Route through cargo_race_retry so a concurrent-build race (os error 2 /
        # failed to exec) retries once and, if persistent, fails the leg (nonzero
        # exit) instead of letting nextest's Summary count exec-failures as real
        # failures — parse_rust_results then classifies the leg ERRORED.
        cargo_race_retry "$out" cargo nextest run --no-fail-fast "$@"
    else
        cargo_race_retry "$out" cargo test "$@"
    fi
}

run_rust_workspace() {
    echo "=== Running Rust unit tests (workspace) ==="
    # --lib --bins --tests excludes doctests: doctests compile under rustdoc at
    # RUN time (not prebuildable by `--no-run`), so they live in
    # run_rust_doctests as the lone compiling invocation of the parallel phase.
    if rust_test_leg "$RUST_OUTPUT" --workspace --exclude ori_llvm --lib --bins --tests; then
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
    if rust_test_leg "$RUST_RT_OUTPUT" -p ori_rt; then
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
    if rust_test_leg "$RUST_LLVM_OUTPUT" -p ori_llvm --lib; then
        echo "  ✓ Rust LLVM tests passed"
        return 0
    else
        echo "  ✗ Rust LLVM tests FAILED"
        return 1
    fi
}

run_aot() {
    echo "=== Running AOT integration tests (gated burden probe) ==="
    # The AOT RC / floor verdict is captured ONLY under the gated burden probe:
    # ORI_DISABLE_PREDICATE_STACK_RC=1 switches the legacy predicate-stack RC
    # emitter OFF so the burden path is the sole RC emitter (the path of record,
    # under active migration). The DEFAULT path runs the predicate stack, which is
    # false-green on known-failing floor cells AND can double-free in the
    # predicate-stack/burden coexistence surface — that abort produces no
    # parseable nextest summary, which is why the default-path aot leg ERRORED.
    # diagnostics/aot-guardrail.sh --floor uses the same gated env. Subshell scopes
    # the env to this leg only.
    if (
        export ORI_DISABLE_PREDICATE_STACK_RC=1 ORI_VERIFY_ARC=1 ORI_VERIFY_EACH=1
        rust_test_leg "$AOT_OUTPUT" -p ori_llvm --test aot
    ); then
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
    # ONE invocation: machine-readable JSON to a file, stderr to the capture.
    # The console summary is reconstructed from the JSON (no second text run).
    mkdir -p "$(dirname "$ORI_INTERP_JSON")"
    "$CARGO_TARGET_DIR"/debug/ori test --format json $INCREMENTAL_FLAG tests/ > "$ORI_INTERP_JSON" 2>"$ORI_INTERP_OUTPUT"
    local exit_code=$?
    if [ $exit_code -eq 0 ]; then
        python3 "$PARSE_TEST_JSON" --summary-line "$ORI_INTERP_JSON" | sed 's/^/  /'
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
            # Write a valid empty-summary JSON so parse_ori_results reads zeros
            # cleanly (a missing/empty file would trip the parse-error fallback).
            mkdir -p "$(dirname "$ORI_LLVM_JSON")"
            printf '%s\n' '{"files":[],"passed":0,"failed":0,"skipped":0,"skipped_unchanged":0,"llvm_compile_fail":0,"error_files":0,"llvm_compile_fail_files":0,"duration_ns":0}' > "$ORI_LLVM_JSON"
            return 0
            ;;
    esac
    # Assumes LLVM release build (target/release/ori + libori_rt.a) was done in a prior phase.
    # ONE invocation: machine-readable JSON to a file, stderr to the capture
    # (so a crash diagnostic survives when stdout carries no parseable JSON).
    mkdir -p "$(dirname "$ORI_LLVM_JSON")"
    "$CARGO_TARGET_DIR"/release/ori test --format json --backend=llvm $INCREMENTAL_FLAG tests/ > "$ORI_LLVM_JSON" 2>"$ORI_LLVM_OUTPUT"
    local exit_code=$?
    if [ $exit_code -eq 0 ]; then
        python3 "$PARSE_TEST_JSON" --summary-line "$ORI_LLVM_JSON" | sed 's/^/  /'
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
    local passed failed ignored

    if [ -n "$NEXTEST_ACTIVE" ] && grep -qE "$BUILD_RACE_RE" "$output_file" 2>/dev/null; then
        # Concurrent-build race (os error 2 / failed to exec) in the nextest
        # output: the `[double-spawn] failed to exec` exec-failures are counted
        # as `failed` in nextest's Summary line, so parsing that count serializes
        # a phantom regression. The aggregate Summary cannot separate exec-
        # failures from real failures, so force the errored sentinel: zero the
        # counts and let the leg's nonzero exit drive suite_status -> errored (a
        # non-verdict / INCOMPLETE), never a `failed` count. rust_test_leg already
        # retried once via cargo_race_retry; reaching here means the race persisted.
        echo "  ⚠ ${prefix}: build-artifact race (os error 2) in nextest output — classifying leg ERRORED (non-verdict), not parsing the contaminated Summary" >&2
        passed=0; failed=0; ignored=0
    elif grep -qE "Summary \[" "$output_file" 2>/dev/null; then
        # cargo-nextest aggregate summary line, e.g.
        #   Summary [   0.090s] 296 tests run: 296 passed, 0 skipped
        #   Summary [   1.2s ] 100 tests run: 95 passed, 3 failed, 2 skipped
        # nextest uses "skipped" (not "ignored"); map skipped -> IGNORED to feed
        # the same ${prefix}_IGNORED consumer.
        local summary
        summary=$(grep -E "Summary \[" "$output_file" 2>/dev/null | tail -1)
        passed=$(echo "$summary" | grep -oE '[0-9]+ passed' | grep -oE '^[0-9]+' | head -1)
        failed=$(echo "$summary" | grep -oE '[0-9]+ failed' | grep -oE '^[0-9]+' | head -1)
        ignored=$(echo "$summary" | grep -oE '[0-9]+ skipped' | grep -oE '^[0-9]+' | head -1)
        passed=${passed:-0}; failed=${failed:-0}; ignored=${ignored:-0}
    elif [ -n "$NEXTEST_ACTIVE" ] && ! grep -qE "^test result:" "$output_file" 2>/dev/null; then
        # nextest was active but produced NEITHER a Summary line NOR a cargo
        # `test result:` line — the output is unparseable (crash / build error /
        # partial). Per the parse-error contract this is a HARD suite failure,
        # NEVER a silent zero that masks dropped tests.
        echo "  ✗ ${prefix}: nextest produced no parseable summary (parse-error) — failing the suite" >&2
        passed=0; failed=1; ignored=0
    else
        # plain `cargo test` human summary: "test result: ok. N passed; M failed; K ignored; ..."
        passed=$(grep -E "^test result:" "$output_file" 2>/dev/null | grep -oE '[0-9]+ passed' | awk '{sum += $1} END {print sum+0}')
        failed=$(grep -E "^test result:" "$output_file" 2>/dev/null | sed 's/.*; \([0-9]*\) failed.*/\1/' | awk '{sum += $1} END {print sum+0}')
        ignored=$(grep -E "^test result:" "$output_file" 2>/dev/null | sed 's/.*; \([0-9]*\) ignored.*/\1/' | awk '{sum += $1} END {print sum+0}')
    fi

    eval "${prefix}_PASSED=$passed"
    eval "${prefix}_FAILED=$failed"
    eval "${prefix}_IGNORED=$ignored"
}

# Populate ${prefix}_{PASSED,FAILED,SKIPPED,LCFAIL,CRASHED} from the runner's
# `--format json` object via diagnostics/parse_test_json.py. The text scrape of
# the human summary line is retired; the helper json.loads the same pinned
# schema the runner serializes.
#
# Parse-error fallback (per the JSON parse-error contract): a non-zero helper
# exit (malformed JSON: crash, stderr leakage, partial output) sets the counts
# to a HARD-failure sentinel (FAILED=1, never bare-empty which breaks `$(( ))`)
# AND propagates to ${prefix}_EXIT so ANY_CORE_FAILED catches it. NEVER a silent
# default-zero — default-zero is what masked the malformed-output bug.
parse_ori_results() {
    local json_file=$1
    local prefix=$2
    local exit_code=$3  # Pass exit code to detect crashes

    # Check for crash (signal-terminated process) — keys on exit code, not
    # output text; unchanged by the JSON migration.
    if [ "${exit_code:-0}" -gt 128 ]; then
        eval "${prefix}_PASSED=0"
        eval "${prefix}_FAILED=0"
        eval "${prefix}_SKIPPED=0"
        eval "${prefix}_LCFAIL=0"
        eval "${prefix}_CRASHED=1"
        return
    fi

    local counts
    if ! counts=$(python3 "$PARSE_TEST_JSON" --counts "$json_file" 2>/dev/null); then
        # Malformed runner JSON: mark a hard suite failure and force the run red.
        echo "  ✗ ${prefix}: runner emitted invalid JSON (parse-error) — failing the suite" >&2
        eval "${prefix}_PASSED=0"
        eval "${prefix}_FAILED=1"
        eval "${prefix}_SKIPPED=0"
        eval "${prefix}_LCFAIL=0"
        eval "${prefix}_CRASHED=0"
        eval "${prefix}_EXIT=1"
        return
    fi

    # Helper emits KEY=value lines (PASSED/FAILED/SKIPPED/LCFAIL/CRASHED).
    local key value
    while IFS='=' read -r key value; do
        [ -n "$key" ] && eval "${prefix}_${key}=${value}"
    done <<< "$counts"
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
    stat -c '%d:%i:%Y:%s' "$CARGO_TARGET_DIR"/debug/ori "$CARGO_TARGET_DIR"/debug/libori_rt.a 2>/dev/null \
        || stat -f '%d:%i:%m:%z' "$CARGO_TARGET_DIR"/debug/ori "$CARGO_TARGET_DIR"/debug/libori_rt.a 2>/dev/null \
        || echo "absent"
}

# Stage manifest the AOT harness publishes while snapshotting its per-process
# artifacts (staged_artifacts_dir in compiler/ori_llvm/tests/aot/util/binary.rs).
# Deleted at baseline capture, so post-leg presence proves THIS run staged.
# Consumed by aot_snapshot_verdict() (scripts/aot_gate_lib.sh).
AOT_STAGE_MANIFEST="build/aot-stage-manifest-debug.txt"

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

# Build cache: wrap every cargo build below with sccache when it is on PATH, so
# the debug + release rebuilds reuse cached object files. Guarded on `command -v
# sccache`: absent -> RUSTC_WRAPPER stays unset and builds proceed unwrapped
# (graceful degrade, never a failure). Exported once here so both the parallel
# and serial build branches inherit it.
if command -v sccache >/dev/null 2>&1; then
    export RUSTC_WRAPPER=sccache
    # sccache cannot cache incrementally-compiled crates; debug builds default to
    # CARGO_INCREMENTAL=1, so without this the wrapper caches nothing (compile
    # requests executed = 0). Disabling cargo incremental lets sccache cache + hit.
    export CARGO_INCREMENTAL=0
    echo "=== Build cache: sccache active (RUSTC_WRAPPER=sccache, CARGO_INCREMENTAL=0) ==="
else
    echo "=== Build cache: sccache absent — builds proceed unwrapped ==="
fi

# Fast linker (mold) for THIS runner's cargo builds only — scoped to test-all.sh
# (the section's mission target) rather than a global .cargo/config.toml so it
# never changes how unrelated cargo invocations link. Guarded on `command -v
# mold`: -fuse-ld=mold ERRORS at link if mold is absent (not a silent fallback),
# so the flag is added ONLY when mold is detected; absent -> default linker.
if command -v mold >/dev/null 2>&1; then
    export RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }-C link-arg=-fuse-ld=mold"
    echo "=== Fast linker: mold active (-fuse-ld=mold) ==="
else
    echo "=== Fast linker: mold absent — default linker ==="
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
    echo "=== Test runner: cargo-nextest absent — cargo test legs ==="
fi

# Incremental spec-test skip: pass --incremental to the Ori spec-test runner so
# it skips targets unchanged since the last run (the runner already supports
# config.incremental for both the interpreter and LLVM legs). ORI_TEST_FORCE_FULL=1
# empties the flag to force a full run (every target executes).
INCREMENTAL_FLAG="--incremental"
if [ -n "${ORI_TEST_FORCE_FULL:-}" ]; then
    INCREMENTAL_FLAG=""
    echo "=== Incremental: ORI_TEST_FORCE_FULL set — full run (no --incremental) ==="
else
    echo "=== Incremental: --incremental active (unchanged targets skipped) ==="
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
    _PHASE1_T0=$(date +%s.%N)
    PRECOMPILE_OUTPUT=$(mktemp)
    if cargo_race_retry "$PRECOMPILE_OUTPUT" cargo test --no-run -q --workspace --lib --bins --tests; then
        echo "  ✓ Debug test binaries pre-built"
    else
        echo -e "  ${RED}✗ Debug test-binary pre-build FAILED${NC}"
        cat "$PRECOMPILE_OUTPUT"
    fi
    # nextest builds its OWN test harness binaries (distinct from cargo test's),
    # so when active they must ALSO be pre-built serially here — a Phase-2 plain
    # `cargo nextest run` would otherwise compile and reintroduce the rmeta race.
    if [ -n "$NEXTEST_ACTIVE" ]; then
        if cargo_race_retry "$PRECOMPILE_OUTPUT" cargo nextest run --no-run --workspace --lib --bins --tests; then
            echo "  ✓ nextest test binaries pre-built"
        else
            echo -e "  ${RED}✗ nextest test-binary pre-build FAILED${NC}"
            cat "$PRECOMPILE_OUTPUT"
        fi
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
    awk "BEGIN { printf \"%.1f\", $(date +%s.%N) - ${_PHASE1_T0} }" > "$LEG_TIMING_DIR/phase1_build" 2>/dev/null

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

# Per-leg wall-time breakdown (additive observability). In parallel mode the
# legs OVERLAP, so the slowest leg (top row) is the critical path that sets the
# Phase-2 wall time; phase1_build is the serial pre-build ahead of Phase 2.
if compgen -G "$LEG_TIMING_DIR/*" > /dev/null 2>&1; then
    echo ""
    echo -e "${BOLD}Per-leg wall time (seconds, slowest first):${NC}"
    for _tf in "$LEG_TIMING_DIR"/*; do
        printf '%s %s\n' "$(cat "$_tf" 2>/dev/null)" "$(basename "$_tf")"
    done | sort -rn | awk '{ printf "  %8.1fs  %s\n", $1, $2 }'
    echo ""
fi

# Show verbose output if requested or on failure
# AOT identity gate (snapshot-integrity) — decide BEFORE any failure dump:
# invalidated per-test output is noise, not signal. aot_snapshot_verdict
# (scripts/aot_gate_lib.sh) re-stats each staged file at <stage-dir>/<name>
# against its manifest-recorded identity. Verdicts:
#   valid       -> the per-PID hardlink snapshot stayed intact; mid-run target/
#                  churn (test-all's own concurrent rebuilds) is harmless by
#                  design and never invalidates
#   invalid:*   -> a staged file went missing or drifted mid-leg
#   fallback    -> no stage manifest published (snapshot staging never ran);
#                  conservative live compare against the pre-run baseline
AOT_INVALID=0
AOT_VERDICT=$(aot_snapshot_verdict "$AOT_STAGE_MANIFEST" "${AOT_ARTIFACT_BASELINE:-}")
case "$AOT_VERDICT" in
    valid)
        : # snapshot intact — AOT counts trustworthy
        ;;
    invalid:*)
        AOT_INVALID=1
        echo ""
        echo -e "${RED}AOT LEG INVALID - the staged AOT snapshot was corrupted mid-leg (${AOT_VERDICT#invalid:}); AOT counts are not trustworthy${NC}"
        ;;
    fallback)
        if [ -n "${AOT_ARTIFACT_BASELINE:-}" ] && [ "$(artifact_identity)" != "$AOT_ARTIFACT_BASELINE" ]; then
            AOT_INVALID=1
            echo ""
            echo -e "${RED}AOT LEG INVALID - build artifacts changed mid-run and no stage manifest was published (snapshot staging never ran); AOT counts are not trustworthy${NC}"
        fi
        ;;
esac

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
    # Display-only; non-fatal under set -e (parse-error fallback owns marking).
    { [ -f "$ORI_INTERP_JSON" ] && python3 "$PARSE_TEST_JSON" --fail-lines "$ORI_INTERP_JSON" 2>/dev/null; } || true
    cat "$ORI_INTERP_OUTPUT"
    echo ""
    echo "--- Ori LLVM tests ---"
    { [ -f "$ORI_LLVM_JSON" ] && python3 "$PARSE_TEST_JSON" --fail-lines "$ORI_LLVM_JSON" 2>/dev/null; } || true
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
        # Per-failure detail reconstructed from the runner JSON; stderr (crash
        # diagnostics) appended after. Display-only: a malformed-JSON helper exit
        # must NOT abort the run under set -e — the parse-error fallback in
        # parse_ori_results (below) owns detection + suite marking + emit_json.
        { [ -f "$ORI_INTERP_JSON" ] && python3 "$PARSE_TEST_JSON" --fail-lines "$ORI_INTERP_JSON" 2>/dev/null; } || true
        cat "$ORI_INTERP_OUTPUT"
    fi
    if [[ $ORI_LLVM_EXIT -ne 0 ]]; then
        echo ""
        echo -e "${RED}--- Ori LLVM test failures ---${NC}"
        # Display-only; non-fatal under set -e (parse-error fallback owns marking).
        { [ -f "$ORI_LLVM_JSON" ] && python3 "$PARSE_TEST_JSON" --fail-lines "$ORI_LLVM_JSON" 2>/dev/null; } || true
        cat "$ORI_LLVM_OUTPUT"
    fi
fi

# Parse all results
parse_rust_results "$RUST_OUTPUT" "RUST"
parse_rust_results "$RUST_RT_OUTPUT" "RUST_RT"
parse_rust_results "$RUST_LLVM_OUTPUT" "RUST_LLVM"
parse_rust_results "$AOT_OUTPUT" "AOT"
parse_rust_results "$DOCTEST_OUTPUT" "DOCTEST"
parse_ori_results "$ORI_INTERP_JSON" "ORI_INTERP" "$ORI_INTERP_EXIT"
# LLVM JSON exists only when run_ori_llvm ran (or Windows-skip wrote an empty
# summary). When the LLVM release build failed, run_ori_llvm is never dispatched
# and ORI_LLVM_EXIT is already 1 (rendered "BUILD FAILED"); skip parsing so a
# legitimately-absent file does not trip the parse-error fallback.
if [ -f "$ORI_LLVM_JSON" ]; then
    parse_ori_results "$ORI_LLVM_JSON" "ORI_LLVM" "$ORI_LLVM_EXIT"
fi

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
# Rust (cargo/libtest) failures: scrape `test <name> ... FAILED` lines from the
# cargo text log via diagnostics/parse_test_json.py --rust-failures. The helper
# is the single JSON-emission home (json.dumps escapes control chars by
# construction), so the harness carries no hand-rolled bash string escaper.
# Emits a single-line JSON array on stdout. cargo/libtest output is NOT the Ori
# runner JSON; it keeps its text scrape per the migration boundary.
rust_failures_json() {
    local output_file="$1"
    local suite_id="$2"
    [ ! -f "$output_file" ] && { echo '[]'; return; }
    python3 "$PARSE_TEST_JSON" --rust-failures --suite "$suite_id" "$output_file" 2>/dev/null \
        || echo '[]'
}

# Per-test failure scrape gated on the suite's final status. An `errored` suite
# (nonzero exit, no parseable summary) has untrustworthy per-test results, so it
# contributes NO entries to failures[] — the suite-level ERR (per_suite + totals
# + the table row) is the SSOT. A non-errored suite scrapes normally. Mirrors the
# errored re-derivation json_suite_full applies to per_suite.
scrape_failures_unless_errored() {
    local output_file="$1" suite_id="$2" status="$3"
    if [ "$status" = "errored" ]; then
        printf '[]'
        return
    fi
    rust_failures_json "$output_file" "$suite_id"
}

# Ori failures: read the runner's --format json per-test payload via
# diagnostics/parse_test_json.py --failures-json. Supersedes the bash escaper
# entirely (the runner serde-escapes failure messages; json.dumps re-escapes on
# emit). Emits a single-line JSON array on stdout.
ori_failures_json() {
    local json_file="$1"
    local suite_id="$2"
    [ ! -f "$json_file" ] && { echo '[]'; return; }
    python3 "$PARSE_TEST_JSON" --failures-json --suite "$suite_id" "$json_file" 2>/dev/null \
        || echo '[]'
}

# Per-node verdict: feed the assembled failures[] array (each carrying
# source_path + leak_positive from parse_test_json) into per_node_verdict.py,
# which attributes each failure to its owning plan-node/bug via the path->node
# index and emits the per_node block. Mirrors ori_failures_json's shell style.
# Input is the full JSON array text on stdin; output is the per_node object.
per_node_verdict_json() {
    local failures_array="$1"
    printf '%s' "$failures_array" \
        | python3 "$PER_NODE_VERDICT" 2>/dev/null \
        || echo '{}'
}

# Strip the surrounding [ ] of a single-line JSON array, yielding its inner
# objects (or empty for `[]`). The helper emits compact single-line arrays.
json_array_inner() {
    local arr="$1"
    arr="${arr#\[}"
    arr="${arr%\]}"
    printf '%s' "$arr"
}

emit_json() {
    local path="$1"
    local overall="passed"
    if [ "$ANY_FAILED" -ne 0 ]; then
        overall="failed"
    fi

    # Parse individual failures from each suite log
    local rust_failures ori_interp_failures ori_llvm_failures rt_failures rust_llvm_failures aot_failures
    rust_failures=$(scrape_failures_unless_errored "$RUST_OUTPUT" "rust_workspace" "$RUST_STATUS")
    rt_failures=$(scrape_failures_unless_errored "$RUST_RT_OUTPUT" "rust_rt" "$RUST_RT_STATUS")
    rust_llvm_failures=$(scrape_failures_unless_errored "$RUST_LLVM_OUTPUT" "rust_llvm" "$RUST_LLVM_STATUS")
    local doctest_failures
    doctest_failures=$(scrape_failures_unless_errored "$DOCTEST_OUTPUT" "rust_doctest" "$DOCTEST_STATUS")
    aot_failures=$(scrape_failures_unless_errored "$AOT_OUTPUT" "aot" "$AOT_STATUS")
    ori_interp_failures=$(ori_failures_json "$ORI_INTERP_JSON" "ori_interp")
    ori_llvm_failures=$(ori_failures_json "$ORI_LLVM_JSON" "ori_llvm")

    # Combine all failures: strip each array's outer [ ], join the inner objects
    # with commas. Each helper emits a compact single-line JSON array.
    local all_failures="" inner
    for failures in "$rust_failures" "$rt_failures" "$rust_llvm_failures" "$doctest_failures" "$aot_failures" "$ori_interp_failures" "$ori_llvm_failures"; do
        inner=$(json_array_inner "$failures")
        if [ -n "$inner" ]; then
            if [ -n "$all_failures" ]; then
                all_failures+=",$inner"
            else
                all_failures="$inner"
            fi
        fi
    done

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

    # Per-node verdict block, attributed from the assembled failures[]. Build
    # the bracketed array text per_node_verdict.py expects on stdin.
    local per_node_block
    per_node_block=$(per_node_verdict_json "[$all_failures]")

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
        echo "  \"head_sha\": \"$(git -C "$(dirname "$0")" rev-parse HEAD 2>/dev/null || echo unknown)\","
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
        echo "  \"per_node\": $per_node_block,"
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
ANY_CORE_FAILED=$((RUST_EXIT + DOCTEST_EXIT + RUST_RT_EXIT + RUST_LLVM_EXIT + AOT_EXIT + WASM_EXIT + ORI_INTERP_EXIT))
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
