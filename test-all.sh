#!/bin/bash
# Run ALL tests: Rust unit tests and Ori language tests
# Usage: ./test-all [-v|--verbose] [-s|--sequential]
#
# This script runs:
# 1. Rust unit tests (workspace crates)
# 2. Rust unit tests (LLVM crates: ori_llvm, ori_rt)
# 3. WASM playground build check
# 4. Ori language spec tests (interpreter backend)
# 5. Ori language spec tests (LLVM backend)
#
# By default, runs tests in parallel for faster execution.
# Use -s or --sequential for sequential execution.
# Use -v or --verbose to see all output.

set -e

# Check for flags
VERBOSE=0
PARALLEL=1
for arg in "$@"; do
    case $arg in
        -v|--verbose)
            VERBOSE=1
            ;;
        -s|--sequential)
            PARALLEL=0
            ;;
    esac
done

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BOLD='\033[1m'
NC='\033[0m' # No Color

# Temp files for capturing output
RUST_OUTPUT=$(mktemp)
RUST_LLVM_OUTPUT=$(mktemp)
WASM_OUTPUT=$(mktemp)
ORI_INTERP_OUTPUT=$(mktemp)
ORI_LLVM_OUTPUT=$(mktemp)

# Cleanup temp files on exit
cleanup() {
    rm -f "$RUST_OUTPUT" "$RUST_LLVM_OUTPUT" "$WASM_OUTPUT" "$ORI_INTERP_OUTPUT" "$ORI_LLVM_OUTPUT"
}
trap cleanup EXIT

# Track failures
RUST_EXIT=0
RUST_LLVM_EXIT=0
WASM_EXIT=0
ORI_INTERP_EXIT=0
ORI_LLVM_EXIT=0

# --- Test runner functions ---

run_rust_workspace() {
    echo "=== Running Rust unit tests (workspace) ==="
    if cargo test --workspace 2>&1 > "$RUST_OUTPUT"; then
        echo "  ✓ Rust workspace tests passed"
        return 0
    else
        echo "  ✗ Rust workspace tests FAILED"
        return 1
    fi
}

run_rust_llvm() {
    echo "=== Running Rust unit tests (LLVM crates) ==="
    # AOT integration tests (spec.rs, cli.rs) invoke the `ori` binary as a
    # subprocess. They need an LLVM-enabled build at target/release/ori.
    # Without this, parallel workspace tests may overwrite target/debug/ori
    # with a non-LLVM build, causing all AOT tests to fail.
    cargo build -p oric -p ori_rt --features llvm --release -q 2>/dev/null || true
    if cargo test --manifest-path compiler/ori_llvm/Cargo.toml 2>&1 > "$RUST_LLVM_OUTPUT"; then
        echo "  ✓ Rust LLVM tests passed"
        return 0
    else
        echo "  ✗ Rust LLVM tests FAILED"
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
    # Always run Ori tests in verbose mode to show skip reasons
    if cargo run -p oric --bin ori -- test --verbose tests/ 2>&1 > "$ORI_INTERP_OUTPUT"; then
        grep -E "[0-9]+ passed, [0-9]+ failed" "$ORI_INTERP_OUTPUT" | tail -1 | sed 's/^/  /'
        return 0
    else
        echo "  ✗ Ori interpreter tests FAILED"
        return 1
    fi
}

run_ori_llvm() {
    echo "=== Running Ori language tests (LLVM backend) ==="
    # Build compiler with LLVM feature AND runtime library
    cargo build -p oric -p ori_rt --features llvm --release -q 2>/dev/null || true
    # Always run Ori tests in verbose mode to show skip reasons
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
        eval "${prefix}_XFAIL=0"
        eval "${prefix}_CRASHED=1"
        return
    fi

    local line=$(grep -E "[0-9]+ passed, [0-9]+ failed" "$output_file" 2>/dev/null | tail -1)
    local nums=($(echo "$line" | grep -oE '[0-9]+'))

    eval "${prefix}_PASSED=${nums[0]:-0}"
    eval "${prefix}_FAILED=${nums[1]:-0}"
    eval "${prefix}_SKIPPED=${nums[2]:-0}"
    eval "${prefix}_CRASHED=0"

    # Extract xfail count (appears as "N expected failures" in summary)
    local xfail=$(echo "$line" | grep -oP '[0-9]+(?= expected failures)' || echo "0")
    eval "${prefix}_XFAIL=${xfail:-0}"
}

# --- Main execution ---

if [[ $PARALLEL -eq 1 ]]; then
    echo -e "${BOLD}Running tests in parallel...${NC}"
    echo ""

    # Phase 1: Non-LLVM tests + LLVM build (in parallel)
    # Start LLVM build early — Phase 2 tests depend on it
    cargo build -p oric -p ori_rt --features llvm --release -q 2>/dev/null &
    LLVM_BUILD_PID=$!

    run_rust_workspace &
    RUST_PID=$!

    run_wasm_build &
    WASM_PID=$!

    # Wait for phase 1
    wait $RUST_PID || RUST_EXIT=1
    wait $WASM_PID || WASM_EXIT=1

    echo ""

    # Phase 2: LLVM-dependent tests + interpreter (in parallel)
    # Wait for LLVM build — AOT integration tests need target/release/ori
    wait $LLVM_BUILD_PID 2>/dev/null || true

    run_rust_llvm &
    RUST_LLVM_PID=$!

    run_ori_interpreter &
    ORI_INTERP_PID=$!

    run_ori_llvm &
    ORI_LLVM_PID=$!

    # Wait for phase 2
    wait $RUST_LLVM_PID || RUST_LLVM_EXIT=1
    ORI_INTERP_EXIT=0
    wait $ORI_INTERP_PID || ORI_INTERP_EXIT=$?
    ORI_LLVM_EXIT=0
    wait $ORI_LLVM_PID || ORI_LLVM_EXIT=$?

else
    # Sequential execution
    echo -e "${BOLD}Running tests sequentially...${NC}"
    echo ""

    run_rust_workspace || RUST_EXIT=1
    echo ""
    run_rust_llvm || RUST_LLVM_EXIT=1
    echo ""
    run_wasm_build || WASM_EXIT=1
    echo ""
    ORI_INTERP_EXIT=0
    run_ori_interpreter || ORI_INTERP_EXIT=$?
    echo ""
    ORI_LLVM_EXIT=0
    run_ori_llvm || ORI_LLVM_EXIT=$?
fi

# Show verbose output if requested or on failure
if [[ $VERBOSE -eq 1 ]]; then
    echo ""
    echo "=== Detailed Output ==="
    echo ""
    echo "--- Rust workspace tests ---"
    cat "$RUST_OUTPUT"
    echo ""
    echo "--- Rust LLVM tests ---"
    cat "$RUST_LLVM_OUTPUT"
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
    if [[ $RUST_LLVM_EXIT -ne 0 ]]; then
        echo ""
        echo -e "${RED}--- Rust LLVM test failures ---${NC}"
        cat "$RUST_LLVM_OUTPUT"
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
parse_rust_results "$RUST_LLVM_OUTPUT" "RUST_LLVM"
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
printf "%-30s %8s %8s %8s %8s\n" "Test Suite" "Passed" "Failed" "Skipped" "XFail"
printf "%-30s %8s %8s %8s %8s\n" "------------------------------" "--------" "--------" "--------" "--------"
printf "%-30s %8d %8d %8d %8s\n" "Rust unit tests (workspace)" "$RUST_PASSED" "$RUST_FAILED" "$RUST_IGNORED" "-"
printf "%-30s %8d %8d %8d %8s\n" "Rust unit tests (LLVM)" "$RUST_LLVM_PASSED" "$RUST_LLVM_FAILED" "$RUST_LLVM_IGNORED" "-"
printf "%-30s %8s\n" "WASM playground build" "$WASM_STATUS"
printf "%-30s %8d %8d %8d %8s\n" "Ori spec (interpreter)" "$ORI_INTERP_PASSED" "$ORI_INTERP_FAILED" "$ORI_INTERP_SKIPPED" "-"
if [ "${ORI_LLVM_CRASHED:-0}" -eq 1 ]; then
    printf "%-30s %8s\n" "Ori spec (LLVM backend)" "CRASHED"
else
    printf "%-30s %8d %8d %8d %8d\n" "Ori spec (LLVM backend)" "$ORI_LLVM_PASSED" "$ORI_LLVM_FAILED" "$ORI_LLVM_SKIPPED" "${ORI_LLVM_XFAIL:-0}"
fi
printf "%-30s %8s %8s %8s %8s\n" "------------------------------" "--------" "--------" "--------" "--------"

# Calculate totals
TOTAL_PASSED=$((RUST_PASSED + RUST_LLVM_PASSED + ORI_INTERP_PASSED + ORI_LLVM_PASSED))
TOTAL_FAILED=$((RUST_FAILED + RUST_LLVM_FAILED + ORI_INTERP_FAILED + ORI_LLVM_FAILED))
TOTAL_SKIPPED=$((RUST_IGNORED + RUST_LLVM_IGNORED + ORI_INTERP_SKIPPED + ORI_LLVM_SKIPPED))
TOTAL_XFAIL=$((${ORI_LLVM_XFAIL:-0}))

printf "${BOLD}%-30s %8d %8d %8d %8d${NC}\n" "TOTAL" "$TOTAL_PASSED" "$TOTAL_FAILED" "$TOTAL_SKIPPED" "$TOTAL_XFAIL"
echo ""

# Final status
ANY_FAILED=$((RUST_EXIT + RUST_LLVM_EXIT + WASM_EXIT + ORI_INTERP_EXIT + ORI_LLVM_EXIT))

if [ "$ANY_FAILED" -eq 0 ]; then
    echo -e "${GREEN}${BOLD}=== All tests passed ===${NC}"
    exit 0
else
    echo -e "${RED}${BOLD}=== Some tests failed ===${NC}"
    exit 1
fi
