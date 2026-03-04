#!/bin/bash
# Run ALL tests: Rust unit tests and Ori language tests
# Usage: ./test-all [-v|--verbose] [-s|--sequential] [--json[=<path>]]
#
# This script runs:
# 1. Rust unit tests (workspace default members — excludes ori_llvm)
# 2. Runtime library tests (ori_rt)
# 3. Rust unit tests (ori_llvm)
# 4. AOT integration tests (compile-and-run through ori build)
# 5. WASM playground build check
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
    esac
done

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
AOT_OUTPUT=$(mktemp)
WASM_OUTPUT=$(mktemp)
ORI_INTERP_OUTPUT=$(mktemp)
ORI_LLVM_OUTPUT=$(mktemp)

# Cleanup temp files on exit
cleanup() {
    rm -f "$RUST_OUTPUT" "$RUST_RT_OUTPUT" "$RUST_LLVM_OUTPUT" "$AOT_OUTPUT" "$WASM_OUTPUT" "$ORI_INTERP_OUTPUT" "$ORI_LLVM_OUTPUT"
}
trap cleanup EXIT

# Track failures
RUST_EXIT=0
RUST_RT_EXIT=0
RUST_LLVM_EXIT=0
AOT_EXIT=0
WASM_EXIT=0
ORI_INTERP_EXIT=0
ORI_LLVM_EXIT=0

# --- Test runner functions ---

run_rust_workspace() {
    echo "=== Running Rust unit tests (workspace) ==="
    if cargo test --workspace --exclude ori_llvm 2>&1 > "$RUST_OUTPUT"; then
        echo "  ✓ Rust workspace tests passed"
        return 0
    else
        echo "  ✗ Rust workspace tests FAILED"
        return 1
    fi
}

run_rust_rt() {
    echo "=== Running runtime library tests (ori_rt) ==="
    if cargo test -p ori_rt 2>&1 > "$RUST_RT_OUTPUT"; then
        echo "  ✓ Runtime library tests passed"
        return 0
    else
        echo "  ✗ Runtime library tests FAILED"
        return 1
    fi
}

run_rust_llvm() {
    echo "=== Running Rust unit tests (ori_llvm) ==="
    # Run ori_llvm lib unit tests + doc-tests (AOT integration tests run separately below)
    if cargo test -p ori_llvm --lib 2>&1 > "$RUST_LLVM_OUTPUT" && \
       cargo test -p ori_llvm --doc 2>&1 >> "$RUST_LLVM_OUTPUT"; then
        echo "  ✓ Rust LLVM tests passed"
        return 0
    else
        echo "  ✗ Rust LLVM tests FAILED"
        return 1
    fi
}

run_aot() {
    echo "=== Running AOT integration tests ==="
    if cargo test -p ori_llvm --test aot 2>&1 > "$AOT_OUTPUT"; then
        echo "  ✓ AOT integration tests passed"
        return 0
    else
        echo "  ✗ AOT integration tests FAILED"
        return 1
    fi
}

run_wasm_build() {
    echo "=== Checking WASM playground builds ==="
    if ! rustup target list --installed | grep -q wasm32-unknown-unknown; then
        echo "  (skipped - wasm32-unknown-unknown target not installed)"
        echo "skipped" > "$WASM_OUTPUT"
        return 0
    fi
    if cargo build --manifest-path website/playground-wasm/Cargo.toml --target wasm32-unknown-unknown --release 2>&1 > "$WASM_OUTPUT"; then
        echo "  ✓ WASM build passed"
        return 0
    else
        echo "  ✗ WASM build FAILED"
        return 1
    fi
}

run_ori_interpreter() {
    echo "=== Running Ori language tests (interpreter) ==="
    # Use pre-built binary directly to avoid cargo lock contention.
    # target/debug/ori exists after workspace tests compile oric.
    if ./target/debug/ori test --verbose tests/ 2>&1 > "$ORI_INTERP_OUTPUT"; then
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

    local passed=$(grep -E "^test result:" "$output_file" 2>/dev/null | sed 's/.*ok\. \([0-9]*\) passed.*/\1/' | awk '{sum += $1} END {print sum+0}')
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

# --- Main execution ---

if [[ $PARALLEL -eq 1 ]]; then
    echo -e "${BOLD}Running tests in parallel...${NC}"
    echo ""

    # Phase 1: Workspace tests + WASM build (different target dirs, no lock contention)
    # WASM uses website/playground-wasm/target/ (its own [workspace])
    run_rust_workspace &
    RUST_PID=$!

    run_wasm_build &
    WASM_PID=$!

    wait $RUST_PID || RUST_EXIT=1
    wait $WASM_PID || WASM_EXIT=1

    echo ""

    # Phase 2: LLVM release build (sequential — shares target/ with workspace)
    echo "=== Building LLVM release binary ==="
    LLVM_BUILD_OK=1
    if ! cargo build -p oric -p ori_rt --release -q 2>&1; then
        echo -e "  ${RED}✗ LLVM release build FAILED — skipping LLVM spec tests${NC}"
        LLVM_BUILD_OK=0
        ORI_LLVM_EXIT=1
    fi

    echo ""

    # Phase 3: All remaining tests in parallel (no cargo lock contention)
    # - run_rust_rt: -p ori_rt (unit tests)
    # - run_rust_llvm: -p ori_llvm --lib (unit tests)
    # - run_aot: -p ori_llvm --test aot (AOT integration tests)
    # - run_ori_interpreter: direct binary (./target/debug/ori), no cargo
    # - run_ori_llvm: direct binary (./target/release/ori), no cargo
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
    if [[ $AOT_EXIT -ne 0 ]]; then
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
parse_ori_results "$ORI_INTERP_OUTPUT" "ORI_INTERP"
parse_ori_results "$ORI_LLVM_OUTPUT" "ORI_LLVM" "$ORI_LLVM_EXIT"

# Determine WASM status
if grep -q "skipped" "$WASM_OUTPUT" 2>/dev/null; then
    WASM_STATUS="skipped"
elif [[ $WASM_EXIT -eq 0 ]]; then
    WASM_STATUS="passed"
else
    WASM_STATUS="FAILED"
fi

# --- Print Summary ---
echo ""
echo "=============================================="
echo -e "${BOLD}                TEST SUMMARY${NC}"
echo "=============================================="
echo ""
printf "%-30s %8s %8s %8s %8s\n" "Test Suite" "Passed" "Failed" "Skipped" "LCFail"
printf "%-30s %8s %8s %8s %8s\n" "------------------------------" "--------" "--------" "--------" "--------"
printf "%-30s %8d %8d %8d %8s\n" "Rust unit tests (workspace)" "$RUST_PASSED" "$RUST_FAILED" "$RUST_IGNORED" "-"
printf "%-30s %8d %8d %8d %8s\n" "Runtime library (ori_rt)" "$RUST_RT_PASSED" "$RUST_RT_FAILED" "$RUST_RT_IGNORED" "-"
printf "%-30s %8d %8d %8d %8s\n" "Rust unit tests (ori_llvm)" "$RUST_LLVM_PASSED" "$RUST_LLVM_FAILED" "$RUST_LLVM_IGNORED" "-"
printf "%-30s %8d %8d %8d %8s\n" "AOT integration tests" "$AOT_PASSED" "$AOT_FAILED" "$AOT_IGNORED" "-"
printf "%-30s %8s\n" "WASM playground build" "$WASM_STATUS"
printf "%-30s %8d %8d %8d %8s\n" "Ori spec (interpreter)" "$ORI_INTERP_PASSED" "$ORI_INTERP_FAILED" "$ORI_INTERP_SKIPPED" "-"
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
TOTAL_PASSED=$((RUST_PASSED + RUST_RT_PASSED + RUST_LLVM_PASSED + AOT_PASSED + ORI_INTERP_PASSED + ORI_LLVM_PASSED))
TOTAL_FAILED=$((RUST_FAILED + RUST_RT_FAILED + RUST_LLVM_FAILED + AOT_FAILED + ORI_INTERP_FAILED + ORI_LLVM_FAILED))
TOTAL_SKIPPED=$((RUST_IGNORED + RUST_RT_IGNORED + RUST_LLVM_IGNORED + AOT_IGNORED + ORI_INTERP_SKIPPED + ORI_LLVM_SKIPPED))
TOTAL_LCFAIL=$((${ORI_LLVM_LCFAIL:-0}))

printf "${BOLD}%-30s %8d %8d %8d %8d${NC}\n" "TOTAL" "$TOTAL_PASSED" "$TOTAL_FAILED" "$TOTAL_SKIPPED" "$TOTAL_LCFAIL"
echo ""

# --- Emit JSON if requested ---
emit_json() {
    local path="$1"
    local overall="passed"
    if [ "$ANY_FAILED" -ne 0 ]; then
        overall="failed"
    fi

    # Helper: emit a numeric suite as a JSON object
    # Args: name passed failed skipped [lcfail]
    json_suite() {
        local lcfail="${5:-null}"
        printf '    { "name": "%s", "passed": %d, "failed": %d, "skipped": %d, "lcfail": %s }' \
            "$1" "$2" "$3" "$4" "$lcfail"
    }

    # Helper: emit WASM status-only suite
    json_wasm_suite() {
        printf '    { "name": "WASM playground build", "status": "%s", "passed": null, "failed": null, "skipped": null, "lcfail": null }' \
            "$1"
    }

    # Helper: emit crashed suite
    json_crashed_suite() {
        printf '    { "name": "%s", "status": "crashed", "passed": null, "failed": null, "skipped": null, "lcfail": null }' \
            "$1"
    }

    {
        echo "{"
        echo "  \"timestamp\": \"$(date -u +%Y-%m-%dT%H:%M:%SZ)\","
        echo "  \"overall\": \"$overall\","
        echo "  \"suites\": ["
        json_suite "Rust unit tests (workspace)" "$RUST_PASSED" "$RUST_FAILED" "$RUST_IGNORED"
        echo ","
        json_suite "Runtime library (ori_rt)" "$RUST_RT_PASSED" "$RUST_RT_FAILED" "$RUST_RT_IGNORED"
        echo ","
        json_suite "Rust unit tests (ori_llvm)" "$RUST_LLVM_PASSED" "$RUST_LLVM_FAILED" "$RUST_LLVM_IGNORED"
        echo ","
        json_suite "AOT integration tests" "$AOT_PASSED" "$AOT_FAILED" "$AOT_IGNORED"
        echo ","
        json_wasm_suite "$WASM_STATUS"
        echo ","
        json_suite "Ori spec (interpreter)" "$ORI_INTERP_PASSED" "$ORI_INTERP_FAILED" "$ORI_INTERP_SKIPPED"
        echo ","
        if [ "${LLVM_BUILD_OK:-1}" -eq 0 ]; then
            json_crashed_suite "Ori spec (LLVM backend)"
        elif [ "${ORI_LLVM_CRASHED:-0}" -eq 1 ]; then
            json_crashed_suite "Ori spec (LLVM backend)"
        else
            json_suite "Ori spec (LLVM backend)" "$ORI_LLVM_PASSED" "$ORI_LLVM_FAILED" "$ORI_LLVM_SKIPPED" "$ORI_LLVM_LCFAIL"
        fi
        echo ""
        echo "  ],"
        echo "  \"totals\": { \"passed\": $TOTAL_PASSED, \"failed\": $TOTAL_FAILED, \"skipped\": $TOTAL_SKIPPED, \"lcfail\": $TOTAL_LCFAIL }"
        echo "}"
    } > "$path"

    echo "Test results written to $path"
}

# Final status
ANY_FAILED=$((RUST_EXIT + RUST_RT_EXIT + RUST_LLVM_EXIT + AOT_EXIT + WASM_EXIT + ORI_INTERP_EXIT + ORI_LLVM_EXIT))

if [[ $EMIT_JSON -eq 1 ]]; then
    emit_json "$JSON_PATH"
fi

if [ "$ANY_FAILED" -eq 0 ]; then
    echo -e "${GREEN}${BOLD}=== All tests passed ===${NC}"
    exit 0
else
    echo -e "${RED}${BOLD}=== Some tests failed ===${NC}"
    exit 1
fi
