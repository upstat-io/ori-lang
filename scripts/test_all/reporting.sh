# Human summary and finalization helpers for test-all.sh.

print_leg_timings() {
    if compgen -G "$LEG_TIMING_DIR/*" > /dev/null 2>&1; then
        echo ""
        echo -e "${BOLD}Per-leg wall time (seconds, slowest first):${NC}"
        for _tf in "$LEG_TIMING_DIR"/*; do
            printf '%s %s\n' "$(cat "$_tf" 2>/dev/null)" "$(basename "$_tf")"
        done | sort -rn | awk '{ printf "  %8.1fs  %s\n", $1, $2 }'
        echo ""
    fi
}

apply_aot_snapshot_verdict() {
    AOT_INVALID=0
    AOT_VERDICT=$(aot_snapshot_verdict "$AOT_STAGE_MANIFEST" "${AOT_ARTIFACT_BASELINE:-}")
    case "$AOT_VERDICT" in
        valid)
            : # snapshot intact
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
}

show_suite_outputs() {
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
        { [ -f "$ORI_INTERP_JSON" ] && python3 "$PARSE_TEST_JSON" --fail-lines "$ORI_INTERP_JSON" 2>/dev/null; } || true
        cat "$ORI_INTERP_OUTPUT"
        echo ""
        echo "--- Ori LLVM tests ---"
        { [ -f "$ORI_LLVM_JSON" ] && python3 "$PARSE_TEST_JSON" --fail-lines "$ORI_LLVM_JSON" 2>/dev/null; } || true
        cat "$ORI_LLVM_OUTPUT"
    else
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
            { [ -f "$ORI_INTERP_JSON" ] && python3 "$PARSE_TEST_JSON" --fail-lines "$ORI_INTERP_JSON" 2>/dev/null; } || true
            cat "$ORI_INTERP_OUTPUT"
        fi
        if [[ $ORI_LLVM_EXIT -ne 0 ]]; then
            echo ""
            echo -e "${RED}--- Ori LLVM test failures ---${NC}"
            { [ -f "$ORI_LLVM_JSON" ] && python3 "$PARSE_TEST_JSON" --fail-lines "$ORI_LLVM_JSON" 2>/dev/null; } || true
            cat "$ORI_LLVM_OUTPUT"
        fi
    fi
}

collect_suite_results() {
    parse_rust_results "$RUST_OUTPUT" "RUST"
    parse_rust_results "$RUST_RT_OUTPUT" "RUST_RT"
    parse_rust_results "$RUST_LLVM_OUTPUT" "RUST_LLVM"
    parse_rust_results "$AOT_OUTPUT" "AOT"
    parse_rust_results "$DOCTEST_OUTPUT" "DOCTEST"
    parse_ori_results "$ORI_INTERP_JSON" "ORI_INTERP" "$ORI_INTERP_EXIT"
    if [ -f "$ORI_LLVM_JSON" ]; then
        parse_ori_results "$ORI_LLVM_JSON" "ORI_LLVM" "$ORI_LLVM_EXIT"
    fi

    AOT_LEAKS=$(grep -c "leaked memory" "$AOT_OUTPUT" 2>/dev/null || true)
    AOT_LEAKS=${AOT_LEAKS:-0}
    if [ "${AOT_INVALID:-0}" = "1" ]; then
        AOT_FAILED=0
        AOT_PASSED=0
        AOT_IGNORED=0
        AOT_EXIT=1
    fi

    if grep -q "skipped" "$WASM_OUTPUT" 2>/dev/null; then
        WASM_STATUS="skipped"
    elif [[ $WASM_EXIT -eq 0 ]]; then
        WASM_STATUS="passed"
    else
        WASM_STATUS="FAILED"
    fi
}

compute_suite_statuses() {
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
    if [ "${LLVM_BUILD_OK:-1}" -eq 0 ] || [ "${ORI_LLVM_CRASHED:-0}" -eq 1 ]; then
        INCOMPLETE_SUITES=$((INCOMPLETE_SUITES + 1))
    fi
}

print_rust_row() {
    local name="$1" passed="$2" failed="$3" skipped="$4" status="$5" extra="${6:-}"
    if [ "$status" = "errored" ]; then
        printf "%-30s %8s %8s %8s %8s  ${RED}<- build/run failed (no results)${NC}\n" \
            "$name" "ERR" "ERR" "ERR" "-"
    else
        printf "%-30s %8d %8d %8d %8s%s\n" "$name" "$passed" "$failed" "$skipped" "-" "$extra"
    fi
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

    TOTAL_PASSED=$((DOCTEST_PASSED + RUST_PASSED + RUST_RT_PASSED + RUST_LLVM_PASSED + AOT_PASSED + ORI_INTERP_PASSED + ORI_LLVM_PASSED))
    TOTAL_FAILED=$((DOCTEST_FAILED + RUST_FAILED + RUST_RT_FAILED + RUST_LLVM_FAILED + AOT_FAILED + ORI_INTERP_FAILED + ORI_LLVM_FAILED))
    TOTAL_SKIPPED=$((DOCTEST_IGNORED + RUST_IGNORED + RUST_RT_IGNORED + RUST_LLVM_IGNORED + AOT_IGNORED + ORI_INTERP_SKIPPED + ORI_LLVM_SKIPPED))
    TOTAL_LCFAIL=$((${ORI_LLVM_LCFAIL:-0}))

    printf "${BOLD}%-30s %8d %8d %8d %8d${NC}\n" "TOTAL" "$TOTAL_PASSED" "$TOTAL_FAILED" "$TOTAL_SKIPPED" "$TOTAL_LCFAIL"
    if [ "$INCOMPLETE_SUITES" -gt 0 ]; then
        echo -e "${RED}${BOLD}TOTAL IS INCOMPLETE: $INCOMPLETE_SUITES suite(s) errored before producing counts — the real failure count is higher.${NC}"
    fi
    echo ""

    if [ "$AOT_LEAKS" -gt 0 ]; then
        echo -e "${YELLOW}${BOLD}⚠  $AOT_LEAKS AOT test(s) leaked memory (ORI_CHECK_LEAKS=1 detected RC leaks)${NC}"
        echo ""
    fi

    if [[ $EMIT_JSON -eq 0 ]] && [[ $RUST_LLVM_EXIT -ne 0 || $AOT_EXIT -ne 0 || $ORI_LLVM_EXIT -ne 0 ]]; then
        echo "  Diagnostic hints:"
        echo "    diagnose-aot.sh <file.ori>      — all-in-one AOT diagnostic"
        echo "    dual-exec-debug.sh <file.ori>   — compare interpreter vs AOT"
        echo "    bisect-passes.sh <file.ori>     — identify failing AIMS phase"
        echo "    codegen-audit.sh <file.ori>     — static RC/COW/ABI check"
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
