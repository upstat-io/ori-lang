# Post-run collection helpers for test-all.sh.
# shellcheck shell=bash

# AOT identity-gate fallback baseline: two-file combined stat fingerprint used
# by apply_aot_snapshot_verdict()'s "fallback" branch when no per-process stage
# manifest was published. Manifest path is the AOT gate's other input.
artifact_identity() {
    printf '%s\n%s\n' \
        "$(aot_artifact_identity_of_path "$CARGO_TARGET_DIR/debug/ori")" \
        "$(aot_artifact_identity_of_path "$CARGO_TARGET_DIR/debug/libori_rt.a")"
}

AOT_STAGE_MANIFEST="build/aot-stage-manifest-debug.txt"

capture_runtime_aot_gate() {
    local attempt_dir="$1" baseline="$2"
    RUNTIME_AOT_POST_IDENTITY=$(artifact_identity)
    RUNTIME_AOT_SNAPSHOT_VERDICT=$(aot_snapshot_verdict "$AOT_STAGE_MANIFEST" "$baseline")
    RUNTIME_AOT_MANIFEST_DIGEST=""
    if [ -f "$AOT_STAGE_MANIFEST" ]; then
        aot_gate_command cp "$AOT_STAGE_MANIFEST" "$attempt_dir/aot-stage-manifest.txt"
        local digest_line
        digest_line=$(aot_gate_command sha256sum "$attempt_dir/aot-stage-manifest.txt")
        RUNTIME_AOT_MANIFEST_DIGEST=${digest_line%% *}
    fi
}

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
        # shellcheck disable=SC2034 # read by reporting.sh:print_test_summary + json_report.sh:emit_json
        AOT_PASSED=0
        # shellcheck disable=SC2034 # read by reporting.sh:print_test_summary + json_report.sh:emit_json
        AOT_IGNORED=0
        AOT_EXIT=1
    fi

    if grep -q "skipped" "$WASM_OUTPUT" 2>/dev/null; then
        # shellcheck disable=SC2034 # read by reporting.sh:print_test_summary + json_report.sh:emit_json
        WASM_STATUS="skipped"
    elif [[ $WASM_EXIT -eq 0 ]]; then
        # shellcheck disable=SC2034 # read by reporting.sh:print_test_summary + json_report.sh:emit_json
        WASM_STATUS="passed"
    else
        # shellcheck disable=SC2034 # read by reporting.sh:print_test_summary + json_report.sh:emit_json
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
