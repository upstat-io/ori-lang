#!/bin/bash
# Regression: test-all reports exact AOT + LLVM/JIT RC evidence and keeps
# nextest process leaks separate. Phrase matching is not an acceptable oracle.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
PARSER="$ROOT/diagnostics/parse_test_leaks.py"
RESULT_PARSER="$ROOT/diagnostics/parse_test_json.py"
POST_RUN="$ROOT/scripts/test_all/post_run.sh"
REPORTING="$ROOT/scripts/test_all/reporting.sh"
JSON_REPORT="$ROOT/scripts/test_all/json_report.sh"
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

cat > "$TMP/aot.log" <<'EOF'
        FAIL [   0.041s] (1/3) ori_llvm::aot direct_probe
  stderr ───
    ori: 2 RC allocation(s) not freed (memory leak)
        FAIL [   0.042s] (2/3) ori_llvm::aot unlabeled_probe
  stderr ───
    ori: 3 RC allocation(s) not freed
        LEAK [   0.043s] (3/3) ori_llvm::aot process_probe
     Summary [   0.100s] 3 tests run: 1 passed, 2 failed
EOF

cat > "$TMP/rust.log" <<'EOF'
        PASS [   0.010s] (1/2) ori_rt clean_probe
        LEAK [   0.020s] (2/2) ori_rt process_probe
     Summary [   0.100s] 2 tests run: 2 passed, 0 skipped
EOF

cat > "$TMP/cargo-test.log" <<'EOF'
test clean_probe ... ok
test result: ok. 1 passed; 0 failed; 0 ignored
EOF

cat > "$TMP/unparseable-leak.log" <<'EOF'
        FAIL [   0.010s] (1/1) ori_llvm::aot unknown_probe
  stderr ───
    leaked memory
     Summary [   0.100s] 1 test run: 0 passed, 1 failed
EOF

cat > "$TMP/llvm.json" <<'EOF'
{
  "files": [{
    "path": "tests/spec/leaks.ori",
    "results": [
      {"name": "first", "outcome": {"Failed": "ARC leak: 4 allocation(s) not freed"}},
      {"name": "second", "outcome": {"Failed": "ARC leak: 1 allocation(s) not freed"}}
    ],
    "errors": []
  }]
}
EOF

[ "$(python3 "$PARSER" --nextest "$TMP/aot.log")" = "2 5 1 1 1" ] || {
    echo "FAIL: AOT leak metrics are not exact" >&2
    exit 1
}
[ "$(python3 "$PARSER" --ori-json "$TMP/llvm.json")" = "2 5 1 0 0" ] || {
    echo "FAIL: LLVM/JIT leak metrics are not exact" >&2
    exit 1
}
[ "$(python3 "$PARSER" --nextest "$TMP/rust.log")" = "0 0 1 1 1" ] || {
    echo "FAIL: Rust nextest process-leak metrics are not exact" >&2
    exit 1
}
[ "$(python3 "$PARSER" --nextest "$TMP/cargo-test.log")" = "0 0 0 0 0" ] || {
    echo "FAIL: cargo test without nextest was reported as a checked process oracle" >&2
    exit 1
}
if python3 "$PARSER" --nextest "$TMP/unparseable-leak.log" >/dev/null 2>&1; then
    echo "FAIL: unparseable RC leak evidence was silently reported as zero" >&2
    exit 1
fi
python3 "$RESULT_PARSER" --rust-failures --suite rust_rt "$TMP/rust.log" > "$TMP/rust-failures.json"
python3 - "$TMP/rust-failures.json" <<'PY'
import json
import sys

failures = json.load(open(sys.argv[1], encoding="utf-8"))
assert failures == [{
    "test_id": "process_probe",
    "test_id_kind": "rust",
    "suite": "rust_rt",
    "failure_kind": "process_leak",
    "error_message": "LEAK ori_rt process_probe (nextest)",
}]
PY

(
    # Exercise the legacy JSON producer, not only its parser helpers.
    PARSE_TEST_JSON="$RESULT_PARSER"
    source "$JSON_REPORT"
    ANY_FAILED=1
    RUST_OUTPUT="$TMP/rust.log"; RUST_RT_OUTPUT="$TMP/cargo-test.log"
    RUST_LLVM_OUTPUT="$TMP/cargo-test.log"; DOCTEST_OUTPUT="$TMP/cargo-test.log"
    AOT_OUTPUT="$TMP/aot.log"; ORI_INTERP_JSON="$TMP/llvm.json"; ORI_LLVM_JSON="$TMP/llvm.json"
    RUST_PASSED=2; RUST_FAILED=0; RUST_IGNORED=0; RUST_STATUS=failed; RUST_EXIT=1
    RUST_RT_PASSED=1; RUST_RT_FAILED=0; RUST_RT_IGNORED=0; RUST_RT_STATUS=passed; RUST_RT_EXIT=0
    RUST_LLVM_PASSED=1; RUST_LLVM_FAILED=0; RUST_LLVM_IGNORED=0; RUST_LLVM_STATUS=passed; RUST_LLVM_EXIT=0
    DOCTEST_PASSED=1; DOCTEST_FAILED=0; DOCTEST_IGNORED=0; DOCTEST_STATUS=passed; DOCTEST_EXIT=0
    AOT_PASSED=1; AOT_FAILED=2; AOT_IGNORED=0; AOT_STATUS=failed; AOT_EXIT=1
    ORI_INTERP_PASSED=1; ORI_INTERP_FAILED=0; ORI_INTERP_SKIPPED=0; ORI_INTERP_EXIT=0
    ORI_LLVM_PASSED=0; ORI_LLVM_FAILED=2; ORI_LLVM_SKIPPED=0; ORI_LLVM_LCFAIL=0
    LLVM_BUILD_OK=1; ORI_LLVM_CRASHED=0; WASM_STATUS=passed
    RUST_PROCESS_LEAK_CHECK=1; RUST_PROCESS_LEAK_TESTS=1
    RUST_RT_PROCESS_LEAK_CHECK=0; RUST_RT_PROCESS_LEAK_TESTS=0
    RUST_LLVM_PROCESS_LEAK_CHECK=0; RUST_LLVM_PROCESS_LEAK_TESTS=0
    AOT_RC_LEAK_CHECK=1; AOT_RC_LEAK_TESTS=2; AOT_RC_LEAK_ALLOCATIONS=5
    AOT_PROCESS_LEAK_CHECK=1; AOT_PROCESS_LEAK_TESTS=1; AOT_LEAKS=2
    ORI_LLVM_RC_LEAK_CHECK=1; ORI_LLVM_RC_LEAK_TESTS=2; ORI_LLVM_RC_LEAK_ALLOCATIONS=5
    TOTAL_PASSED=7; TOTAL_FAILED=4; TOTAL_SKIPPED=0; TOTAL_LCFAIL=0
    TOTAL_RC_LEAK_TESTS=4; TOTAL_RC_LEAK_ALLOCATIONS=10
    TOTAL_PROCESS_LEAK_CHECK=1; TOTAL_PROCESS_LEAK_TESTS=2
    emit_json "$TMP/legacy-summary.json" >/dev/null
)
python3 - "$TMP/legacy-summary.json" <<'PY'
import json
import sys

summary = json.load(open(sys.argv[1], encoding="utf-8"))
assert summary["schema_version"] == 3
assert summary["per_suite"]["rust_workspace"]["process_leak_check"] is True
assert summary["per_suite"]["rust_workspace"]["process_leak_tests"] == 1
assert summary["per_suite"]["ori_llvm"]["process_leak_check"] is False
assert summary["totals"]["process_leak_check"] is True
assert summary["totals"]["process_leak_tests"] == 2
PY

if grep -q 'grep -c "leaked memory"' "$POST_RUN"; then
    echo "FAIL: legacy phrase counter still owns AOT leak reporting" >&2
    exit 1
fi
grep -q 'PARSE_TEST_LEAKS' "$POST_RUN"
grep -q 'Observed leak evidence' "$REPORTING"
grep -q 'RUST_PROCESS_LEAK_TESTS' "$REPORTING"
grep -q 'rc_leaked_allocations' "$JSON_REPORT"
grep -q 'process_leak_check' "$JSON_REPORT"
grep -q 'process_leak_tests' "$JSON_REPORT"

echo "PASS: test_test_all_leak_metrics"
