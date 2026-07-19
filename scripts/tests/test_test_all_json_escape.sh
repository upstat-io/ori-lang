#!/usr/bin/env bash
# TDD pin: test-all.sh summary failure entries must stay valid JSON when
# failure names/messages contain control chars, quotes, or backslashes. The
# current production path delegates JSON emission to diagnostics/parse_test_json.py
# (json.dumps), not a hand-rolled bash escaper.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TESTALL="$HERE/../../test-all.sh"
JSON_HELPERS="$HERE/../test_all/json_report.sh"
PARSE_TEST_JSON="$HERE/../../diagnostics/parse_test_json.py"

if [ ! -f "$TESTALL" ]; then
    echo "FAIL: test-all.sh not found at $TESTALL"
    exit 1
fi
if [ ! -f "$PARSE_TEST_JSON" ]; then
    echo "FAIL: parse_test_json.py not found at $PARSE_TEST_JSON"
    exit 1
fi
if [ ! -f "$JSON_HELPERS" ]; then
    echo "FAIL: JSON helper file not found at $JSON_HELPERS"
    exit 1
fi

tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT

ori_json="$tmpdir/ori-runner.json"
ori_out="$tmpdir/ori-failures.json"
python3 - "$ori_json" <<'PY'
import json
import sys

name = 'spec\tname\nwith "quote" and \\backslash'
message = 'a\tb\nc\rd\\e"f\x01g'
obj = {
    "passed": 0,
    "failed": 1,
    "skipped": 0,
    "skipped_unchanged": 0,
    "llvm_compile_fail": 0,
    "error_files": 0,
    "llvm_compile_fail_files": 0,
    "duration_ns": 0,
    "files": [{
        "path": "tests/control_chars.ori",
        "passed": 0,
        "failed": 1,
        "skipped": 0,
        "skipped_unchanged": 0,
        "llvm_compile_fail": 0,
        "duration_ns": 0,
        "errors": [],
        "llvm_compile_error": None,
        "results": [{
            "name": name,
            "targets": ["llvm"],
            "outcome": {"Failed": message},
            "duration_ns": 0,
        }],
    }],
}
with open(sys.argv[1], "w", encoding="utf-8") as fh:
    json.dump(obj, fh, ensure_ascii=False)
PY
python3 "$PARSE_TEST_JSON" --failures-json --suite ori_llvm "$ori_json" > "$ori_out"
python3 - "$ori_out" <<'PY'
import json
import sys

raw = open(sys.argv[1], "rb").read().rstrip(b"\n")
for forbidden in (b"\t", b"\r", b"\n", b"\x01"):
    assert forbidden not in raw, f"raw control byte leaked into JSON: {forbidden!r}"
arr = json.loads(raw.decode("utf-8"))
assert len(arr) == 1, f"expected one Ori failure, got {len(arr)}"
entry = arr[0]
assert entry["test_id"] == 'spec\tname\nwith "quote" and \\backslash'
assert entry["error_message"] == 'a\tb\nc\rd\\e"f\x01g'
assert entry["suite"] == "ori_llvm"
assert entry["source_path"] == "tests/control_chars.ori"
print("OK: parse_test_json --failures-json escapes controls and round-trips Ori failure strings")
PY

rust_log="$tmpdir/rust.log"
rust_out="$tmpdir/rust-failures.json"
printf 'test module::case_with_"quote"_and_\\backslash ... FAILED\n' > "$rust_log"
python3 "$PARSE_TEST_JSON" --rust-failures --suite rust_workspace "$rust_log" > "$rust_out"
python3 - "$rust_out" <<'PY'
import json
import sys

arr = json.load(open(sys.argv[1], encoding="utf-8"))
assert len(arr) == 1, f"expected one Rust failure, got {len(arr)}"
entry = arr[0]
assert entry["test_id"] == 'module::case_with_"quote"_and_\\backslash'
assert entry["suite"] == "rust_workspace"
assert entry["test_id_kind"] == "rust"
print("OK: parse_test_json --rust-failures emits parseable JSON for Rust failure strings")
PY

if grep -qE '^(json_escape_string|parse_rust_failures|parse_ori_failures)\(\)' "$TESTALL"; then
    echo "FAIL: retired bash JSON/text-scrape helpers reappeared in test-all.sh"
    exit 1
fi

rust_body="$(sed -n '/^rust_failures_json()/,/^}/p' "$JSON_HELPERS")"
ori_body="$(sed -n '/^ori_failures_json()/,/^}/p' "$JSON_HELPERS")"
if [ -z "$rust_body" ] || ! printf '%s' "$rust_body" | grep -q -- 'PARSE_TEST_JSON.*--rust-failures'; then
    echo "FAIL: rust_failures_json() is not wired to parse_test_json.py --rust-failures"
    exit 1
fi
if [ -z "$ori_body" ] || ! printf '%s' "$ori_body" | grep -q -- 'PARSE_TEST_JSON.*--failures-json'; then
    echo "FAIL: ori_failures_json() is not wired to parse_test_json.py --failures-json"
    exit 1
fi
echo "OK: test-all.sh failure-summary helpers route through parse_test_json.py"
