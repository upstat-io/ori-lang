# Human summary and finalization helpers for test-all.sh.
# shellcheck shell=bash
# shellcheck disable=SC2153 # Suite counters are assigned by the sourcing harness.

print_rust_row() {
    local name="$1" passed="$2" failed="$3" skipped="$4" status="$5" extra="${6:-}"
    if [ "$status" = "errored" ]; then
        printf "%-30s %8s %8s %8s %8s  ${RED}<- build/run failed (no results)${NC}\n" \
            "$name" "ERR" "ERR" "ERR" "-"
    else
        printf "%-30s %8d %8d %8d %8s%s\n" "$name" "$passed" "$failed" "$skipped" "-" "$extra"
    fi
}

print_leak_row() {
    local name="$1" rc_check="$2" rc_tests="$3" rc_allocations="$4"
    local process_check="$5" process_tests="$6"
    local rc_tests_text="N/A" rc_allocations_text="N/A" process_tests_text="N/A"
    if [ "$rc_check" -eq 1 ]; then
        rc_tests_text="$rc_tests"
        rc_allocations_text="$rc_allocations"
    fi
    if [ "$process_check" -eq 1 ]; then
        process_tests_text="$process_tests"
    fi
    printf "%-30s %10s %12s %12s\n" \
        "$name" "$rc_tests_text" "$rc_allocations_text" "$process_tests_text"
}

print_test_summary() {
    echo ""
    echo "=============================================="
    echo -e "${BOLD}                TEST SUMMARY${NC}"
    echo "=============================================="
    echo ""
    printf "%-30s %8s %8s %8s %8s\n" "Test Suite" "Passed" "Failed" "Skipped" "LCFail"
    printf "%-30s %8s %8s %8s %8s\n" "------------------------------" "--------" "--------" "--------" "--------"
    print_rust_row "Rust unit tests (workspace)" "$RUST_PASSED" "$RUST_FAILED" "$RUST_IGNORED" "$RUST_STATUS"
    print_rust_row "Runtime library (ori_rt)" "$RUST_RT_PASSED" "$RUST_RT_FAILED" "$RUST_RT_IGNORED" "$RUST_RT_STATUS"
    print_rust_row "Rust unit tests (ori_llvm)" "$RUST_LLVM_PASSED" "$RUST_LLVM_FAILED" "$RUST_LLVM_IGNORED" "$RUST_LLVM_STATUS"
    if [ "$AOT_RC_LEAK_TESTS" -gt 0 ] && [ "$AOT_STATUS" != "errored" ]; then
        print_rust_row "AOT integration tests" "$AOT_PASSED" "$AOT_FAILED" "$AOT_IGNORED" "$AOT_STATUS" "$(printf '  %b(%d RC-leak tests / %d allocations)%b' "$YELLOW" "$AOT_RC_LEAK_TESTS" "$AOT_RC_LEAK_ALLOCATIONS" "$NC")"
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
    elif [ "${ORI_LLVM_LEAK_PARSE_ERROR:-0}" -eq 1 ]; then
        printf "%-30s %8s\n" "Ori spec (LLVM backend)" "LEAK METRICS ERROR"
    else
        printf "%-30s %8d %8d %8d %8d\n" "Ori spec (LLVM backend)" "$ORI_LLVM_PASSED" "$ORI_LLVM_FAILED" "$ORI_LLVM_SKIPPED" "${ORI_LLVM_LCFAIL:-0}"
    fi
    printf "%-30s %8s %8s %8s %8s\n" "------------------------------" "--------" "--------" "--------" "--------"

    TOTAL_PASSED=$((DOCTEST_PASSED + RUST_PASSED + RUST_RT_PASSED + RUST_LLVM_PASSED + AOT_PASSED + ORI_INTERP_PASSED + ORI_LLVM_PASSED))
    TOTAL_FAILED=$((DOCTEST_FAILED + RUST_FAILED + RUST_RT_FAILED + RUST_LLVM_FAILED + AOT_FAILED + ORI_INTERP_FAILED + ORI_LLVM_FAILED))
    TOTAL_SKIPPED=$((DOCTEST_IGNORED + RUST_IGNORED + RUST_RT_IGNORED + RUST_LLVM_IGNORED + AOT_IGNORED + ORI_INTERP_SKIPPED + ORI_LLVM_SKIPPED))
    TOTAL_LCFAIL=$((${ORI_LLVM_LCFAIL:-0}))

    printf "${BOLD}%-30s %8d %8d %8d %8d${NC}\n" "TOTAL" "$TOTAL_PASSED" "$TOTAL_FAILED" "$TOTAL_SKIPPED" "$TOTAL_LCFAIL"
    if [ "$INCOMPLETE_SUITES" -gt 0 ]; then
        echo -e "${RED}${BOLD}TOTAL IS INCOMPLETE: $INCOMPLETE_SUITES suite(s) errored before producing counts - the real failure count is higher.${NC}"
    fi
    echo ""

    TOTAL_RC_LEAK_TESTS=$((AOT_RC_LEAK_TESTS + ORI_LLVM_RC_LEAK_TESTS))
    TOTAL_RC_LEAK_ALLOCATIONS=$((AOT_RC_LEAK_ALLOCATIONS + ORI_LLVM_RC_LEAK_ALLOCATIONS))
    TOTAL_PROCESS_LEAK_TESTS=$((RUST_PROCESS_LEAK_TESTS + RUST_RT_PROCESS_LEAK_TESTS + RUST_LLVM_PROCESS_LEAK_TESTS + AOT_PROCESS_LEAK_TESTS))
    TOTAL_PROCESS_LEAK_CHECK=$((RUST_PROCESS_LEAK_CHECK || RUST_RT_PROCESS_LEAK_CHECK || RUST_LLVM_PROCESS_LEAK_CHECK || AOT_PROCESS_LEAK_CHECK))
    echo -e "${BOLD}Observed leak evidence:${NC}"
    printf "%-30s %10s %12s %12s\n" "Suite" "RCLeak" "RCAllocs" "ProcLeak"
    print_leak_row "Rust unit tests (workspace)" 0 0 0 "${RUST_PROCESS_LEAK_CHECK:-0}" "${RUST_PROCESS_LEAK_TESTS:-0}"
    print_leak_row "Runtime library (ori_rt)" 0 0 0 "${RUST_RT_PROCESS_LEAK_CHECK:-0}" "${RUST_RT_PROCESS_LEAK_TESTS:-0}"
    print_leak_row "Rust unit tests (ori_llvm)" 0 0 0 "${RUST_LLVM_PROCESS_LEAK_CHECK:-0}" "${RUST_LLVM_PROCESS_LEAK_TESTS:-0}"
    print_leak_row "AOT integration tests" "${AOT_RC_LEAK_CHECK:-0}" "${AOT_RC_LEAK_TESTS:-0}" "${AOT_RC_LEAK_ALLOCATIONS:-0}" "${AOT_PROCESS_LEAK_CHECK:-0}" "${AOT_PROCESS_LEAK_TESTS:-0}"
    print_leak_row "Rust doctests (workspace)" 0 0 0 0 0
    print_leak_row "External playground WASM" 0 0 0 0 0
    print_leak_row "Ori spec (interpreter)" 0 0 0 0 0
    print_leak_row "Ori spec (LLVM backend)" "${ORI_LLVM_RC_LEAK_CHECK:-0}" "${ORI_LLVM_RC_LEAK_TESTS:-0}" "${ORI_LLVM_RC_LEAK_ALLOCATIONS:-0}" 0 0
    printf "%-30s %10d %12d %12d\n" "TOTAL" "$TOTAL_RC_LEAK_TESTS" "$TOTAL_RC_LEAK_ALLOCATIONS" "$TOTAL_PROCESS_LEAK_TESTS"
    echo "  N/A means that suite did not execute that leak oracle."
    echo ""

    if [ "$TOTAL_RC_LEAK_TESTS" -gt 0 ]; then
        echo -e "${YELLOW}${BOLD}[warn] Observed RC leaks: $TOTAL_RC_LEAK_TESTS test(s), $TOTAL_RC_LEAK_ALLOCATIONS allocation(s) not freed (AOT $AOT_RC_LEAK_TESTS/$AOT_RC_LEAK_ALLOCATIONS, LLVM/JIT $ORI_LLVM_RC_LEAK_TESTS/$ORI_LLVM_RC_LEAK_ALLOCATIONS)${NC}"
        echo ""
    fi
    if [ "$TOTAL_PROCESS_LEAK_TESTS" -gt 0 ]; then
        echo -e "${YELLOW}${BOLD}[warn] $TOTAL_PROCESS_LEAK_TESTS process-leak test(s) (workspace ${RUST_PROCESS_LEAK_TESTS}, ori_rt ${RUST_RT_PROCESS_LEAK_TESTS}, ori_llvm ${RUST_LLVM_PROCESS_LEAK_TESTS}, AOT ${AOT_PROCESS_LEAK_TESTS})${NC}"
        echo ""
    fi

    if [[ $EMIT_JSON -eq 0 ]] && [[ $RUST_LLVM_EXIT -ne 0 || $AOT_EXIT -ne 0 || $ORI_LLVM_EXIT -ne 0 ]]; then
        echo "  Diagnostic hints:"
        echo "    diagnose-aot.sh <file.ori>      - all-in-one AOT diagnostic"
        echo "    dual-exec-debug.sh <file.ori>   - compare interpreter vs AOT"
        echo "    bisect-passes.sh <file.ori>     - identify failing AIMS phase"
        echo "    codegen-audit.sh <file.ori>     - static RC/COW/ABI check"
        echo ""
    fi
}

test_all_final_exit_code() {
    local any_failed="${1:-0}"
    if [ "$any_failed" -eq 0 ]; then
        return 0
    fi
    return 1
}

finalize_test_all() {
    LEAK_FAILURE=0
    if [ "${TOTAL_RC_LEAK_TESTS:-0}" -gt 0 ] || [ "${TOTAL_PROCESS_LEAK_TESTS:-0}" -gt 0 ]; then
        LEAK_FAILURE=1
    fi
    LEAK_PARSE_FAILURE=$((RUST_LEAK_PARSE_ERROR + RUST_RT_LEAK_PARSE_ERROR + RUST_LLVM_LEAK_PARSE_ERROR + AOT_LEAK_PARSE_ERROR + ORI_LLVM_LEAK_PARSE_ERROR))
    ANY_CORE_FAILED=$((RUST_EXIT + DOCTEST_EXIT + RUST_RT_EXIT + RUST_LLVM_EXIT + AOT_EXIT + WASM_EXIT + ORI_INTERP_EXIT))
    ANY_FAILED=$((ANY_CORE_FAILED + ORI_LLVM_EXIT + LEAK_FAILURE + LEAK_PARSE_FAILURE))

    if [ -n "$ERRORED_SUITES" ]; then
        echo -e "${RED}${BOLD}Errored suites - built/ran with no parseable results (counted as FAILED):${NC}"
        echo -e "$ERRORED_SUITES"
        echo "  A suite errors when it cannot build or crashes before printing results."
        echo "  This is the case CI surfaces as failures; it is no longer hidden as 0/0 passed."
        echo ""
    fi

    if [[ $EMIT_JSON -eq 1 ]]; then
        emit_json "$JSON_PATH"
    fi

    if [[ $EMIT_JSON_SUMMARY -eq 1 ]]; then
        emit_json "$JSON_SUMMARY_PATH"
    fi

    if test_all_final_exit_code "$ANY_FAILED"; then
        echo -e "${GREEN}${BOLD}=== All tests passed ===${NC}"
        exit 0
    else
        echo -e "${RED}${BOLD}=== Some tests failed ===${NC}"
        exit 1
    fi
}
