#!/bin/bash
# Bounded leaf actions for scripts.test_all_runtime.
# shellcheck shell=bash

set -euo pipefail

runtime_action_main() {
    local action_kind="${ORI_TESTALL_ACTION_KIND:?missing ORI_TESTALL_ACTION_KIND}"
    local action_id="${ORI_TESTALL_ACTION_ID:?missing ORI_TESTALL_ACTION_ID}"
    local attempt_dir="${ORI_TESTALL_ATTEMPT_DIR:?missing ORI_TESTALL_ATTEMPT_DIR}"
    local timeout_secs="${ORI_TESTALL_ACTION_TIMEOUT_SECS:?missing ORI_TESTALL_ACTION_TIMEOUT_SECS}"
    local leaf_timeout=$((timeout_secs - 5))
    local fragment="$TEST_ALL_DIR/scripts/test_all/runtime_fragment.py"
    local stdout_path="$attempt_dir/stdout.log"
    local stderr_path="$attempt_dir/stderr.log"
    local candidate_path="$attempt_dir/candidate.json"
    mkdir -p "$attempt_dir"
    : > "$stdout_path"
    : > "$stderr_path"

    export CARGO_TERM_COLOR=never NEXTEST_COLOR=never ORI_TEST_FORCE_FULL=1
    export ORI_VERIFY_ARC=1 ORI_VERIFY_EACH=1 CARGO_INCREMENTAL=0
    export AOT_GATE_COMMAND_TIMEOUT_SECS=5
    if command -v mold >/dev/null 2>&1; then
        export RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }-C link-arg=-fuse-ld=mold"
    fi

    runtime_leg_selection "${ORI_TESTALL_SUITE:-}"

    case "$action_kind" in
        harness_selftest)
            timeout --signal=TERM --kill-after=5 "${leaf_timeout}s" bash "${ORI_TESTALL_SELFTEST_PATH:?}" >"$stdout_path" 2>"$stderr_path"
            ;;
        check_debug_flags)
            timeout --signal=TERM --kill-after=5 "${leaf_timeout}s" diagnostics/check-debug-flags.sh --no-color >"$stdout_path" 2>"$stderr_path"
            ;;
        crate_dag)
            timeout --signal=TERM --kill-after=5 "${leaf_timeout}s" python3 scripts/crate-dag-lint.py --warn-only >"$stdout_path" 2>"$stderr_path"
            ;;
        prebuild)
            if [ "${ORI_TESTALL_RUNNER:?}" = cargo ]; then
                timeout --signal=TERM --kill-after=5 "${leaf_timeout}s" cargo test --no-run -q "${RUNTIME_SELECTION[@]}" >"$stdout_path" 2>"$stderr_path"
            else
                command -v cargo-nextest >/dev/null 2>&1 || {
                    echo "cargo-nextest is required for exact resumable partitions; install it and resume run ${ORI_TESTALL_RUN_ID}" >&2
                    return 3
                }
                timeout --signal=TERM --kill-after=5 "${leaf_timeout}s" cargo nextest run --no-run "${RUNTIME_SELECTION[@]}" >"$stdout_path" 2>"$stderr_path"
            fi
            ;;
        build)
            runtime_build "${ORI_TESTALL_BUILD:?}" "$leaf_timeout" "$stdout_path" "$stderr_path"
            ;;
        inventory_nextest)
            runtime_nextest_inventory "$leaf_timeout" "$stdout_path" "$stderr_path" "$fragment"
            ;;
        inventory_ori)
            timeout --signal=TERM --kill-after=5 "${leaf_timeout}s" python3 "$fragment" ori-inventory --root "$TEST_ALL_DIR" --output "${ORI_TESTALL_OUTPUT_PATH:?}" >"$stdout_path" 2>"$stderr_path"
            ;;
        inventory_doctest)
            timeout --signal=TERM --kill-after=5 "${leaf_timeout}s" cargo metadata --no-deps --format-version 1 >"$stdout_path" 2>"$stderr_path"
            timeout --signal=TERM --kill-after=5 "${leaf_timeout}s" python3 "$fragment" doctest-inventory --raw "$stdout_path" --output "${ORI_TESTALL_OUTPUT_PATH:?}" >>"$stdout_path" 2>>"$stderr_path"
            ;;
        test_nextest)
            runtime_test_nextest "$leaf_timeout" "$stdout_path" "$stderr_path" "$candidate_path" "$fragment"
            ;;
        test_ori)
            runtime_test_ori "$leaf_timeout" "$stdout_path" "$stderr_path" "$candidate_path" "$fragment"
            ;;
        test_doctest)
            runtime_test_doctest "$leaf_timeout" "$stdout_path" "$stderr_path" "$candidate_path" "$fragment"
            ;;
        test_wasm)
            runtime_test_wasm "$leaf_timeout" "$stdout_path" "$stderr_path" "$candidate_path" "$fragment"
            ;;
        *)
            echo "unknown runtime action kind: $action_kind" >&2
            return 2
            ;;
    esac
}

runtime_build() {
    local build="$1" timeout_secs="$2" stdout_path="$3" stderr_path="$4"
    case "$build" in
        debug_rt) timeout --signal=TERM --kill-after=5 "${timeout_secs}s" cargo build -p ori_rt -q >"$stdout_path" 2>"$stderr_path" ;;
        debug_oric) timeout --signal=TERM --kill-after=5 "${timeout_secs}s" cargo build -p oric --bin ori -q >"$stdout_path" 2>"$stderr_path" ;;
        release_rt) timeout --signal=TERM --kill-after=5 "${timeout_secs}s" cargo build -p ori_rt --release -q >"$stdout_path" 2>"$stderr_path" ;;
        release_oric) timeout --signal=TERM --kill-after=5 "${timeout_secs}s" cargo build -p oric --bin ori --release -q >"$stdout_path" 2>"$stderr_path" ;;
        *) echo "unknown runtime build: $build" >&2; return 2 ;;
    esac
}

runtime_nextest_inventory() {
    local timeout_secs="$1" stdout_path="$2" stderr_path="$3" fragment="$4"
    local partition=()
    if [ -n "${ORI_TESTALL_PARTITION_INDEX:-}" ]; then
        partition=(--partition "hash:${ORI_TESTALL_PARTITION_INDEX}/${ORI_TESTALL_PARTITION_COUNT}")
    fi
    timeout --signal=TERM --kill-after=5 "${timeout_secs}s" cargo nextest list --message-format json "${partition[@]}" "${RUNTIME_SELECTION[@]}" >"$stdout_path" 2>"$stderr_path"
    parse_runtime_fragment "$timeout_secs" "$fragment" nextest-inventory --raw "$stdout_path" --output "${ORI_TESTALL_OUTPUT_PATH:?}" >>"$stdout_path" 2>>"$stderr_path"
}

runtime_test_nextest() {
    local timeout_secs="$1" stdout_path="$2" stderr_path="$3" candidate_path="$4" fragment="$5"
    local rc=0 baseline="" post="" manifest_digest="" snapshot_verdict=""
    local partition="hash:${ORI_TESTALL_PARTITION_INDEX:?}/${ORI_TESTALL_PARTITION_COUNT:?}"
    if [ "${ORI_TESTALL_SUITE}" = aot ]; then
        rm -f "$AOT_STAGE_MANIFEST"
        baseline=$(artifact_identity)
    fi
    (
        if [ "${ORI_TESTALL_SUITE}" = aot ]; then
            export ORI_DISABLE_PREDICATE_STACK_RC=1 ORI_VERIFY_ARC=1 ORI_VERIFY_EACH=1
        fi
        timeout --signal=TERM --kill-after=5 "${timeout_secs}s" cargo nextest run --color=never --no-fail-fast --retries 0 --status-level all --final-status-level none --partition "$partition" "${RUNTIME_SELECTION[@]}"
    ) >"$stdout_path" 2>&1 || rc=$?
    [ "$rc" -eq 124 ] && : > "$ORI_TESTALL_ATTEMPT_DIR/timed_out"
    if [ "${ORI_TESTALL_SUITE}" = aot ]; then
        capture_runtime_aot_gate "$ORI_TESTALL_ATTEMPT_DIR" "$baseline"
        post="$RUNTIME_AOT_POST_IDENTITY"
        snapshot_verdict="$RUNTIME_AOT_SNAPSHOT_VERDICT"
        manifest_digest="$RUNTIME_AOT_MANIFEST_DIGEST"
    fi
    parse_runtime_fragment "$timeout_secs" "$fragment" nextest-result \
        --expected "$ORI_TESTALL_EXPECTED_PATH" --stdout "$stdout_path" --returncode "$rc" \
        --suite "$ORI_TESTALL_SUITE" --display-name "$ORI_TESTALL_DISPLAY_NAME" --output "$candidate_path" \
        --baseline-identity "$baseline" --post-identity "$post" --manifest-digest "$manifest_digest" \
        --snapshot-verdict "$snapshot_verdict" >>"$stdout_path" 2>>"$stderr_path"
}

runtime_test_ori() {
    local timeout_secs="$1" stdout_path="$2" stderr_path="$3" candidate_path="$4" fragment="$5"
    local rc=0
    local paths=()
    mapfile -d '' paths < <(timeout --signal=TERM --kill-after=5 "${timeout_secs}s" python3 "$fragment" items0 --input "$ORI_TESTALL_EXPECTED_PATH" --field path)
    local backend=()
    local binary="$CARGO_TARGET_DIR/debug/ori"
    if [ "$ORI_TESTALL_SUITE" = ori_llvm ]; then
        case "$(timeout --signal=TERM --kill-after=2 5s uname -s)" in
            MINGW*|MSYS*|CYGWIN*|*NT*) echo "LLVM runtime shards are unavailable on Windows" >&2; return 3 ;;
        esac
        backend=(--backend=llvm)
        binary="$CARGO_TARGET_DIR/release/ori"
    fi
    timeout --signal=TERM --kill-after=5 "${timeout_secs}s" "$binary" test --format json "${backend[@]}" "${paths[@]}" >"$stdout_path" 2>"$stderr_path" || rc=$?
    [ "$rc" -eq 124 ] && : > "$ORI_TESTALL_ATTEMPT_DIR/timed_out"
    parse_runtime_fragment "$timeout_secs" "$fragment" ori-result \
        --expected "$ORI_TESTALL_EXPECTED_PATH" --stdout "$stdout_path" --returncode "$rc" \
        --suite "$ORI_TESTALL_SUITE" --display-name "$ORI_TESTALL_DISPLAY_NAME" --output "$candidate_path" >>"$stdout_path" 2>>"$stderr_path"
}

runtime_test_doctest() {
    local timeout_secs="$1" stdout_path="$2" stderr_path="$3" candidate_path="$4" fragment="$5"
    local rc=0 packages=()
    mapfile -d '' packages < <(timeout --signal=TERM --kill-after=5 "${timeout_secs}s" python3 "$fragment" items0 --input "$ORI_TESTALL_EXPECTED_PATH" --field package)
    [ "${#packages[@]}" -eq 1 ] || { echo "doctest action requires exactly one package" >&2; return 3; }
    timeout --signal=TERM --kill-after=5 "${timeout_secs}s" cargo test -p "${packages[0]}" --doc >"$stdout_path" 2>&1 || rc=$?
    [ "$rc" -eq 124 ] && : > "$ORI_TESTALL_ATTEMPT_DIR/timed_out"
    parse_runtime_fragment "$timeout_secs" "$fragment" doctest-result \
        --expected "$ORI_TESTALL_EXPECTED_PATH" --stdout "$stdout_path" --returncode "$rc" \
        --suite "$ORI_TESTALL_SUITE" --display-name "$ORI_TESTALL_DISPLAY_NAME" --output "$candidate_path" >>"$stdout_path" 2>>"$stderr_path"
}

runtime_test_wasm() {
    local timeout_secs="$1" stdout_path="$2" stderr_path="$3" candidate_path="$4" fragment="$5"
    local rc=0 skipped=0 manifest="../ori-lang-website/playground-wasm/Cargo.toml"
    local installed_targets=""
    installed_targets=$(timeout --signal=TERM --kill-after=5 "${timeout_secs}s" rustup target list --installed) || rc=$?
    if [ "$rc" -ne 0 ]; then
        :
    elif [[ $'\n'"$installed_targets"$'\n' != *$'\nwasm32-unknown-unknown\n'* ]] || [ ! -f "$manifest" ]; then
        skipped=1
    else
        timeout --signal=TERM --kill-after=5 "${timeout_secs}s" cargo build --manifest-path "$manifest" --target wasm32-unknown-unknown --release >"$stdout_path" 2>"$stderr_path" || rc=$?
        [ "$rc" -eq 124 ] && : > "$ORI_TESTALL_ATTEMPT_DIR/timed_out"
    fi
    parse_runtime_fragment "$timeout_secs" "$fragment" wasm-result \
        --expected "$ORI_TESTALL_EXPECTED_PATH" --stdout "$stdout_path" --returncode "$rc" \
        --suite "$ORI_TESTALL_SUITE" --display-name "$ORI_TESTALL_DISPLAY_NAME" --output "$candidate_path" \
        --skipped "$skipped" >>"$stdout_path" 2>>"$stderr_path"
}
