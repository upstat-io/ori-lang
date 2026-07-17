# JSON emission helpers for test-all.sh.
# shellcheck shell=bash

rust_failures_json() {
    local output_file="$1"
    local suite_id="$2"
    [ ! -f "$output_file" ] && { echo '[]'; return; }
    python3 "$PARSE_TEST_JSON" --rust-failures --suite "$suite_id" "$output_file" 2>/dev/null \
        || echo '[]'
}

scrape_failures_unless_errored() {
    local output_file="$1" suite_id="$2" status="$3"
    if [ "$status" = "errored" ]; then
        printf '[]'
        return
    fi
    rust_failures_json "$output_file" "$suite_id"
}

ori_failures_json() {
    local json_file="$1"
    local suite_id="$2"
    [ ! -f "$json_file" ] && { echo '[]'; return; }
    python3 "$PARSE_TEST_JSON" --failures-json --suite "$suite_id" "$json_file" 2>/dev/null \
        || echo '[]'
}

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

    local rust_failures rt_failures rust_llvm_failures doctest_failures aot_failures
    local ori_interp_failures ori_llvm_failures
    rust_failures=$(scrape_failures_unless_errored "$RUST_OUTPUT" "rust_workspace" "$RUST_STATUS")
    rt_failures=$(scrape_failures_unless_errored "$RUST_RT_OUTPUT" "rust_rt" "$RUST_RT_STATUS")
    rust_llvm_failures=$(scrape_failures_unless_errored "$RUST_LLVM_OUTPUT" "rust_llvm" "$RUST_LLVM_STATUS")
    doctest_failures=$(scrape_failures_unless_errored "$DOCTEST_OUTPUT" "rust_doctest" "$DOCTEST_STATUS")
    aot_failures=$(scrape_failures_unless_errored "$AOT_OUTPUT" "aot" "$AOT_STATUS")
    ori_interp_failures=$(ori_failures_json "$ORI_INTERP_JSON" "ori_interp")
    ori_llvm_failures=$(ori_failures_json "$ORI_LLVM_JSON" "ori_llvm")

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

    json_suite_full() {
        local id="$1" display="$2" passed="$3" failed="$4" skipped="$5"
        local lcfail="${6:-0}" status="${7:-passed}" exit_code="${8:-0}"
        local rc_leak_check="${9:-false}" rc_leak_tests="${10:-0}"
        local rc_leaked_allocations="${11:-0}" process_leak_check="${12:-false}"
        local process_leak_tests="${13:-0}"
        if [ "$failed" -gt 0 ]; then
            status="failed"
        elif [ "$process_leak_tests" -gt 0 ]; then
            status="failed"
        elif [ "$exit_code" -ne 0 ] && [ "$status" = "passed" ]; then
            status="errored"
        fi
        printf '    "%s": { "display_name": "%s", "passed": %d, "failed": %d, "skipped": %d, "lcfail": %d, "status": "%s", "failed_attributed": 0, "failed_unattributed": %d, "rc_leak_check": %s, "rc_leak_tests": %d, "rc_leaked_allocations": %d, "process_leak_check": %s, "process_leak_tests": %d }' \
            "$id" "$display" "${passed:-0}" "${failed:-0}" "${skipped:-0}" "${lcfail:-0}" "$status" "${failed:-0}" "$rc_leak_check" "$rc_leak_tests" "$rc_leaked_allocations" "$process_leak_check" "$process_leak_tests"
    }

    local wasm_failed=0 wasm_passed=0
    if [ "$WASM_STATUS" = "passed" ]; then wasm_passed=1; else wasm_failed=1; fi

    local llvm_passed=0 llvm_failed=0 llvm_skipped=0 llvm_lcfail=0 llvm_status="passed"
    local llvm_rc_leak_check=false aot_rc_leak_check=false
    local rust_process_leak_check=false rt_process_leak_check=false
    local rust_llvm_process_leak_check=false aot_process_leak_check=false
    local total_process_leak_check=false
    if [ "${LLVM_BUILD_OK:-1}" -eq 0 ]; then llvm_status="build_failed"
    elif [ "${ORI_LLVM_CRASHED:-0}" -eq 1 ]; then llvm_status="crashed"
    else
        llvm_passed=${ORI_LLVM_PASSED:-0}; llvm_failed=${ORI_LLVM_FAILED:-0}
        llvm_skipped=${ORI_LLVM_SKIPPED:-0}; llvm_lcfail=${ORI_LLVM_LCFAIL:-0}
        [ "$llvm_failed" -gt 0 ] && llvm_status="failed"
        [ "${ORI_LLVM_RC_LEAK_CHECK:-0}" -eq 1 ] && llvm_rc_leak_check=true
    fi
    [ "${AOT_RC_LEAK_CHECK:-0}" -eq 1 ] && aot_rc_leak_check=true
    [ "${RUST_PROCESS_LEAK_CHECK:-0}" -eq 1 ] && rust_process_leak_check=true
    [ "${RUST_RT_PROCESS_LEAK_CHECK:-0}" -eq 1 ] && rt_process_leak_check=true
    [ "${RUST_LLVM_PROCESS_LEAK_CHECK:-0}" -eq 1 ] && rust_llvm_process_leak_check=true
    [ "${AOT_PROCESS_LEAK_CHECK:-0}" -eq 1 ] && aot_process_leak_check=true
    [ "${TOTAL_PROCESS_LEAK_CHECK:-0}" -eq 1 ] && total_process_leak_check=true

    {
        echo "{"
        echo "  \"schema_version\": 3,"
        echo "  \"timestamp\": \"$(date -u +%Y-%m-%dT%H:%M:%SZ)\","
        echo "  \"head_sha\": \"$(git -C "$(dirname "$0")" rev-parse HEAD 2>/dev/null || echo unknown)\","
        echo "  \"overall\": \"$overall\","
        echo "  \"failures\": ["
        if [ -n "$all_failures" ]; then
            echo "$all_failures"
        fi
        echo "  ],"
        echo "  \"per_suite\": {"
        json_suite_full "rust_workspace" "Rust unit tests (workspace)" "$RUST_PASSED" "$RUST_FAILED" "$RUST_IGNORED" 0 "$RUST_STATUS" "$RUST_EXIT" false 0 0 "$rust_process_leak_check" "$RUST_PROCESS_LEAK_TESTS"
        echo ","
        json_suite_full "rust_rt" "Runtime library (ori_rt)" "$RUST_RT_PASSED" "$RUST_RT_FAILED" "$RUST_RT_IGNORED" 0 "$RUST_RT_STATUS" "$RUST_RT_EXIT" false 0 0 "$rt_process_leak_check" "$RUST_RT_PROCESS_LEAK_TESTS"
        echo ","
        json_suite_full "rust_llvm" "Rust unit tests (ori_llvm)" "$RUST_LLVM_PASSED" "$RUST_LLVM_FAILED" "$RUST_LLVM_IGNORED" 0 "$RUST_LLVM_STATUS" "$RUST_LLVM_EXIT" false 0 0 "$rust_llvm_process_leak_check" "$RUST_LLVM_PROCESS_LEAK_TESTS"
        echo ","
        json_suite_full "aot" "AOT integration tests" "$AOT_PASSED" "$AOT_FAILED" "$AOT_IGNORED" 0 "$AOT_STATUS" "$AOT_EXIT" "$aot_rc_leak_check" "$AOT_RC_LEAK_TESTS" "$AOT_RC_LEAK_ALLOCATIONS" "$aot_process_leak_check" "$AOT_PROCESS_LEAK_TESTS"
        echo ","
        json_suite_full "rust_doctest" "Rust doctests (workspace)" "$DOCTEST_PASSED" "$DOCTEST_FAILED" "$DOCTEST_IGNORED" 0 "passed" "$DOCTEST_EXIT"
        echo ","
        printf '    "wasm_playground": { "display_name": "External playground WASM", "passed": %d, "failed": %d, "skipped": 0, "lcfail": 0, "status": "%s", "failed_attributed": 0, "failed_unattributed": %d, "rc_leak_check": false, "rc_leak_tests": 0, "rc_leaked_allocations": 0, "process_leak_check": false, "process_leak_tests": 0 }' "$wasm_passed" "$wasm_failed" "$WASM_STATUS" "$wasm_failed"
        echo ","
        json_suite_full "ori_interp" "Ori spec (interpreter)" "$ORI_INTERP_PASSED" "$ORI_INTERP_FAILED" "$ORI_INTERP_SKIPPED" 0 "passed" "$ORI_INTERP_EXIT"
        echo ","
        printf '    "ori_llvm": { "display_name": "Ori spec (LLVM backend)", "passed": %d, "failed": %d, "skipped": %d, "lcfail": %d, "status": "%s", "failed_attributed": 0, "failed_unattributed": %d, "rc_leak_check": %s, "rc_leak_tests": %d, "rc_leaked_allocations": %d, "process_leak_check": false, "process_leak_tests": 0 }' "$llvm_passed" "$llvm_failed" "$llvm_skipped" "$llvm_lcfail" "$llvm_status" "$llvm_failed" "$llvm_rc_leak_check" "$ORI_LLVM_RC_LEAK_TESTS" "$ORI_LLVM_RC_LEAK_ALLOCATIONS"
        echo ""
        echo "  },"
        echo "  \"totals\": { \"passed\": $TOTAL_PASSED, \"failed\": $TOTAL_FAILED, \"skipped\": $TOTAL_SKIPPED, \"lcfail\": $TOTAL_LCFAIL, \"aot_leaks\": $AOT_LEAKS, \"rc_leak_tests\": $TOTAL_RC_LEAK_TESTS, \"rc_leaked_allocations\": $TOTAL_RC_LEAK_ALLOCATIONS, \"process_leak_check\": $total_process_leak_check, \"process_leak_tests\": $TOTAL_PROCESS_LEAK_TESTS }"
        echo "}"
    } > "$path"

    echo "Test results written to $path"
}
