# Result parsing helpers for test-all.sh.

parse_rust_results() {
    local output_file=$1
    local prefix=$2
    local passed failed ignored

    if [ -n "$NEXTEST_ACTIVE" ] && grep -qE "$BUILD_RACE_RE" "$output_file" 2>/dev/null; then
        echo "  ⚠ ${prefix}: build-artifact race (os error 2) in nextest output — classifying leg ERRORED (non-verdict), not parsing the contaminated Summary" >&2
        passed=0; failed=0; ignored=0
    elif grep -qE "Summary \[" "$output_file" 2>/dev/null; then
        local summary
        summary=$(grep -E "Summary \[" "$output_file" 2>/dev/null | tail -1)
        passed=$(echo "$summary" | grep -oE '[0-9]+ passed' | grep -oE '^[0-9]+' | head -1)
        failed=$(echo "$summary" | grep -oE '[0-9]+ failed' | grep -oE '^[0-9]+' | head -1)
        ignored=$(echo "$summary" | grep -oE '[0-9]+ skipped' | grep -oE '^[0-9]+' | head -1)
        passed=${passed:-0}; failed=${failed:-0}; ignored=${ignored:-0}
    elif [ -n "$NEXTEST_ACTIVE" ] && ! grep -qE "^test result:" "$output_file" 2>/dev/null; then
        echo "  ✗ ${prefix}: nextest produced no parseable summary (parse-error) — failing the suite" >&2
        passed=0; failed=1; ignored=0
    else
        passed=$(grep -E "^test result:" "$output_file" 2>/dev/null | grep -oE '[0-9]+ passed' | awk '{sum += $1} END {print sum+0}')
        failed=$(grep -E "^test result:" "$output_file" 2>/dev/null | sed 's/.*; \([0-9]*\) failed.*/\1/' | awk '{sum += $1} END {print sum+0}')
        ignored=$(grep -E "^test result:" "$output_file" 2>/dev/null | sed 's/.*; \([0-9]*\) ignored.*/\1/' | awk '{sum += $1} END {print sum+0}')
    fi

    eval "${prefix}_PASSED=$passed"
    eval "${prefix}_FAILED=$failed"
    eval "${prefix}_IGNORED=$ignored"
}

parse_ori_results() {
    local json_file=$1
    local prefix=$2
    local exit_code=$3

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
        echo "  ✗ ${prefix}: runner emitted invalid JSON (parse-error) — failing the suite" >&2
        eval "${prefix}_PASSED=0"
        eval "${prefix}_FAILED=1"
        eval "${prefix}_SKIPPED=0"
        eval "${prefix}_LCFAIL=0"
        eval "${prefix}_CRASHED=0"
        eval "${prefix}_EXIT=1"
        return
    fi

    local key value
    while IFS='=' read -r key value; do
        [ -n "$key" ] && eval "${prefix}_${key}=${value}"
    done <<< "$counts"
}

artifact_identity() {
    stat -c '%d:%i:%Y:%s' "$CARGO_TARGET_DIR"/debug/ori "$CARGO_TARGET_DIR"/debug/libori_rt.a 2>/dev/null \
        || stat -f '%d:%i:%m:%z' "$CARGO_TARGET_DIR"/debug/ori "$CARGO_TARGET_DIR"/debug/libori_rt.a 2>/dev/null \
        || echo "absent"
}

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
