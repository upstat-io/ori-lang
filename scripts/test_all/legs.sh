# Test leg execution helpers for test-all.sh.

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

BUILD_RACE_RE='os error 2|could not write output|error: failed to write|No such file or directory \(os error 2\)'

cargo_race_retry() {
    local out="$1"; shift
    local attempt
    for attempt in 1 2; do
        if "$@" > "$out" 2>&1; then
            return 0
        fi
        if [[ $attempt -eq 1 ]] && grep -qE "$BUILD_RACE_RE" "$out"; then
            echo "  [warn] build-artifact race (os error 2) - concurrent cargo on shared target/; retrying once after 5s" >&2
            sleep 5
            continue
        fi
        return 1
    done
    return 1
}

build_with_race_retry() {
    local out
    out=$(mktemp)
    if cargo_race_retry "$out" "$@"; then
        rm -f "$out"
        return 0
    fi
    cat "$out"
    rm -f "$out"
    return 1
}

rust_test_leg() {
    local out="$1"; shift
    if [ -n "$NEXTEST_ACTIVE" ]; then
        cargo_race_retry "$out" cargo nextest run --color=never --no-fail-fast "$@"
    else
        cargo_race_retry "$out" cargo test "$@"
    fi
}

run_rust_workspace() {
    echo "=== Running Rust unit tests (workspace) ==="
    if rust_test_leg "$RUST_OUTPUT" --workspace --exclude ori_llvm --lib --bins --tests; then
        echo "  [ok] Rust workspace tests passed"
        return 0
    else
        echo "  [fail] Rust workspace tests FAILED"
        return 1
    fi
}

run_rust_doctests() {
    echo "=== Running Rust doctests (workspace + ori_llvm) ==="
    if cargo test --workspace --doc > "$DOCTEST_OUTPUT" 2>&1; then
        echo "  [ok] Rust doctests passed"
        return 0
    else
        echo "  [fail] Rust doctests FAILED"
        return 1
    fi
}

run_rust_rt() {
    echo "=== Running runtime library tests (ori_rt) ==="
    if rust_test_leg "$RUST_RT_OUTPUT" -p ori_rt; then
        echo "  [ok] Runtime library tests passed"
        return 0
    else
        echo "  [fail] Runtime library tests FAILED"
        return 1
    fi
}

run_rust_llvm() {
    echo "=== Running Rust unit tests (ori_llvm) ==="
    if rust_test_leg "$RUST_LLVM_OUTPUT" -p ori_llvm --lib; then
        echo "  [ok] Rust LLVM tests passed"
        return 0
    else
        echo "  [fail] Rust LLVM tests FAILED"
        return 1
    fi
}

run_aot() {
    echo "=== Running AOT integration tests (gated burden probe) ==="
    if (
        export ORI_DISABLE_PREDICATE_STACK_RC=1 ORI_VERIFY_ARC=1 ORI_VERIFY_EACH=1
        rust_test_leg "$AOT_OUTPUT" -p ori_llvm --test aot
    ); then
        echo "  [ok] AOT integration tests passed"
        return 0
    else
        echo "  [fail] AOT integration tests FAILED"
        return 1
    fi
}

run_wasm_build() {
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
        echo "  [ok] External playground WASM build passed"
        return 0
    else
        echo "  [fail] External playground WASM build FAILED"
        return 1
    fi
}

run_ori_interpreter() {
    echo "=== Running Ori language tests (interpreter) ==="
    mkdir -p "$(dirname "$ORI_INTERP_JSON")"
    "$CARGO_TARGET_DIR"/debug/ori test --format json $INCREMENTAL_FLAG tests/ > "$ORI_INTERP_JSON" 2>"$ORI_INTERP_OUTPUT"
    local exit_code=$?
    if [ $exit_code -eq 0 ]; then
        python3 "$PARSE_TEST_JSON" --summary-line "$ORI_INTERP_JSON" | sed 's/^/  /'
        return 0
    else
        echo "  [fail] Ori interpreter tests FAILED"
        return 1
    fi
}

run_ori_llvm() {
    echo "=== Running Ori language tests (LLVM backend) ==="
    case "$(uname -s)" in
        MINGW*|MSYS*|CYGWIN*|*NT*)
            echo "  (skipped on Windows - JIT recovery not supported; AOT tests cover LLVM codegen)"
            echo "skipped" > "$ORI_LLVM_OUTPUT"
            mkdir -p "$(dirname "$ORI_LLVM_JSON")"
            printf '%s\n' '{"files":[],"passed":0,"failed":0,"skipped":0,"skipped_unchanged":0,"llvm_compile_fail":0,"error_files":0,"llvm_compile_fail_files":0,"duration_ns":0}' > "$ORI_LLVM_JSON"
            return 0
            ;;
    esac
    mkdir -p "$(dirname "$ORI_LLVM_JSON")"
    "$CARGO_TARGET_DIR"/release/ori test --format json --backend=llvm $INCREMENTAL_FLAG tests/ > "$ORI_LLVM_JSON" 2>"$ORI_LLVM_OUTPUT"
    local exit_code=$?
    if [ $exit_code -eq 0 ]; then
        python3 "$PARSE_TEST_JSON" --summary-line "$ORI_LLVM_JSON" | sed 's/^/  /'
        return 0
    elif [ $exit_code -gt 128 ]; then
        local signal=$((exit_code - 128))
        local error_msg=$(grep -i "error\|panic" "$ORI_LLVM_OUTPUT" | head -1)
        if [ -n "$error_msg" ]; then
            echo "  [fail] Ori LLVM backend CRASHED: $error_msg"
        else
            echo "  [fail] Ori LLVM backend CRASHED (signal $signal)"
        fi
        return 1
    else
        echo "  [fail] Ori LLVM tests FAILED"
        return 1
    fi
}
