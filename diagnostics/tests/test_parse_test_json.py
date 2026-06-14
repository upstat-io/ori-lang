"""Tests for diagnostics/parse_test_json.py — the runner-JSON consumer.

Covers the contract the bash harness depends on:
  - valid JSON object -> correct KEY=value counts
  - invalid / non-object / count-missing JSON -> non-zero exit (NEVER silent
    default-zero, the failure mode the JSON consumption fixes)
  - adversarial failure messages (tab / newline / quote / unicode / control)
    round-trip byte-identically through --failures-json (json.dumps) and back
    through json.loads
  - the reconstructed --summary-line matches print_summary_stats shape
"""

import io
import json
import subprocess
import sys
from pathlib import Path

import pytest

HELPER = Path(__file__).resolve().parent.parent / "parse_test_json.py"

# Inline-import the module so failure-entry round-trip asserts at the Python
# boundary (byte-identity), in addition to the subprocess CLI tests.
sys.path.insert(0, str(HELPER.parent))
import parse_test_json  # noqa: E402


def _run(args, stdin=None):
    return subprocess.run(
        [sys.executable, str(HELPER), *args],
        input=stdin,
        capture_output=True,
        text=True,
    )


def _valid_summary(**overrides):
    obj = {
        "files": [],
        "passed": 10,
        "failed": 2,
        "skipped": 1,
        "skipped_unchanged": 0,
        "llvm_compile_fail": 0,
        "error_files": 0,
        "llvm_compile_fail_files": 0,
        "duration_ns": 123456,
    }
    obj.update(overrides)
    return obj


# --- counts mode ---

def test_counts_valid_json_emits_correct_keys():
    obj = _valid_summary(passed=10, failed=2, skipped=1, llvm_compile_fail=3)
    result = _run(["--counts"], stdin=json.dumps(obj))
    assert result.returncode == 0
    lines = dict(line.split("=", 1) for line in result.stdout.splitlines())
    assert lines == {
        "PASSED": "10",
        "FAILED": "2",
        "SKIPPED": "1",
        "LCFAIL": "3",
        "CRASHED": "0",
    }


def test_counts_from_file_argument(tmp_path):
    path = tmp_path / "results.json"
    path.write_text(json.dumps(_valid_summary(passed=7)), encoding="utf-8")
    result = _run(["--counts", str(path)])
    assert result.returncode == 0
    assert "PASSED=7" in result.stdout.splitlines()


# --- parse-error contract: NON-ZERO, never silent default-zero ---

def test_invalid_json_exits_nonzero():
    result = _run(["--counts"], stdin="this is not json {")
    assert result.returncode != 0
    assert "PASSED=" not in result.stdout
    assert "invalid runner JSON" in result.stderr


def test_partial_then_garbage_json_exits_nonzero():
    # SIGPIPE / stderr-leak shape: a valid prefix followed by garbage.
    result = _run(["--counts"], stdin='{"passed": 3, "failed"')
    assert result.returncode != 0
    assert "PASSED=" not in result.stdout


def test_non_object_json_exits_nonzero():
    result = _run(["--counts"], stdin="[1, 2, 3]")
    assert result.returncode != 0
    assert "expected a JSON object" in result.stderr


def test_object_missing_count_field_exits_nonzero():
    obj = _valid_summary()
    del obj["passed"]
    result = _run(["--counts"], stdin=json.dumps(obj))
    assert result.returncode != 0
    assert "missing required aggregate count field" in result.stderr


def test_count_field_not_integer_exits_nonzero():
    obj = _valid_summary()
    obj["passed"] = "ten"
    result = _run(["--counts"], stdin=json.dumps(obj))
    assert result.returncode != 0
    assert "not an integer" in result.stderr


def test_empty_input_exits_nonzero():
    result = _run(["--counts"], stdin="")
    assert result.returncode != 0


# --- adversarial failure-message round-trip (byte-identical) ---

ADVERSARIAL_MESSAGES = [
    "tab\there",
    "newline\nhere",
    'quote"here',
    "backslash\\here",
    "carriage\rreturn",
    "unicode é中文\U0001f600 here",
    "control\x01\x02\x1f bytes",
    'mixed\t"\n\\é all at once',
]


def _summary_with_failure(message):
    return _valid_summary(
        failed=1,
        files=[{
            "path": "tests/spec/adversarial.ori",
            "results": [{
                "name": "adversarial_test",
                "targets": ["target_fn"],
                "outcome": {"Failed": message},
                "duration_ns": 1,
            }],
            "passed": 0,
            "failed": 1,
            "skipped": 0,
            "skipped_unchanged": 0,
            "llvm_compile_fail": 0,
            "duration_ns": 1,
            "errors": [],
            "llvm_compile_error": False,
        }],
    )


@pytest.mark.parametrize("message", ADVERSARIAL_MESSAGES)
def test_failures_json_roundtrips_byte_identically(message):
    obj = _summary_with_failure(message)
    result = _run(["--failures-json", "--suite", "ori_interp"], stdin=json.dumps(obj))
    assert result.returncode == 0
    # The emitted array MUST be valid JSON ...
    entries = json.loads(result.stdout)
    assert len(entries) == 1
    entry = entries[0]
    # ... and the failure message survives byte-for-byte through the
    # json.dumps escaping that replaces the bash json_escape_string.
    assert entry["error_message"] == message
    assert entry["test_id"] == "adversarial_test"
    assert entry["suite"] == "ori_interp"
    assert entry["failure_kind"] == "assertion_failure"


@pytest.mark.parametrize("message", ADVERSARIAL_MESSAGES)
def test_failure_entries_helper_preserves_message(message):
    obj = _summary_with_failure(message)
    entries = parse_test_json._failure_entries(obj, "ori_interp")
    assert len(entries) == 1
    assert entries[0]["error_message"] == message


def test_failures_json_emits_jq_valid_array_for_empty_failures():
    obj = _valid_summary(failed=0, files=[])
    result = _run(["--failures-json", "--suite", "ori_llvm"], stdin=json.dumps(obj))
    assert result.returncode == 0
    assert json.loads(result.stdout) == []


def test_failures_json_requires_suite():
    obj = _valid_summary()
    result = _run(["--failures-json"], stdin=json.dumps(obj))
    assert result.returncode == 2


def test_failures_json_captures_file_errors_and_llvm_compile_fail():
    obj = _valid_summary(
        failed=1,
        error_files=1,
        files=[{
            "path": "tests/spec/broken.ori",
            "results": [{
                "name": "blocked",
                "targets": [],
                "outcome": {"LlvmCompileFail": "codegen exploded"},
                "duration_ns": 0,
            }],
            "passed": 0,
            "failed": 1,
            "skipped": 0,
            "skipped_unchanged": 0,
            "llvm_compile_fail": 1,
            "duration_ns": 0,
            "errors": ["type error: cannot unify"],
            "llvm_compile_error": True,
        }],
    )
    result = _run(["--failures-json", "--suite", "ori_llvm"], stdin=json.dumps(obj))
    assert result.returncode == 0
    entries = json.loads(result.stdout)
    kinds = sorted(e["failure_kind"] for e in entries)
    assert kinds == ["file_error", "llvm_compile_fail"]


# --- summary-line reconstruction ---

def test_summary_line_basic():
    obj = _valid_summary(passed=10, failed=2, skipped=1)
    result = _run(["--summary-line"], stdin=json.dumps(obj))
    assert result.returncode == 0
    assert result.stdout.strip() == "10 passed, 2 failed, 1 skipped"


def test_summary_line_appends_extra_segments():
    obj = _valid_summary(
        passed=5, failed=0, skipped=0,
        skipped_unchanged=3, llvm_compile_fail=2, error_files=1,
    )
    result = _run(["--summary-line"], stdin=json.dumps(obj))
    assert result.returncode == 0
    assert result.stdout.strip() == (
        "5 passed, 0 failed, 0 skipped, 3 skipped (unchanged), "
        "2 llvm compile fail, 1 files with errors"
    )


def test_summary_line_invalid_json_exits_nonzero():
    result = _run(["--summary-line"], stdin="garbage")
    assert result.returncode != 0


# --- fail-lines reconstruction ---

def test_fail_lines_renders_failed_test():
    obj = _summary_with_failure("assertion failed: 1 == 2")
    result = _run(["--fail-lines"], stdin=json.dumps(obj))
    assert result.returncode == 0
    assert "tests/spec/adversarial.ori" in result.stdout
    assert "FAIL: adversarial_test - assertion failed: 1 == 2" in result.stdout


def test_fail_lines_renders_file_error_and_llvm_compile_fail():
    obj = _valid_summary(
        failed=1,
        files=[{
            "path": "tests/spec/broken.ori",
            "results": [{
                "name": "blocked",
                "targets": [],
                "outcome": {"LlvmCompileFail": "codegen exploded"},
                "duration_ns": 0,
            }],
            "passed": 0, "failed": 1, "skipped": 0, "skipped_unchanged": 0,
            "llvm_compile_fail": 1, "duration_ns": 0,
            "errors": ["type error: cannot unify"], "llvm_compile_error": True,
        }],
    )
    result = _run(["--fail-lines"], stdin=json.dumps(obj))
    assert result.returncode == 0
    assert "LLVM COMPILE FAIL: blocked - codegen exploded" in result.stdout
    assert "ERROR: type error: cannot unify" in result.stdout


def test_fail_lines_empty_for_all_pass():
    obj = _valid_summary(failed=0, files=[])
    result = _run(["--fail-lines"], stdin=json.dumps(obj))
    assert result.returncode == 0
    assert result.stdout.strip() == ""


# --- rust-failures (cargo text scrape; NOT runner JSON) ---

def test_rust_failures_scrapes_failed_lines():
    log = (
        "running 3 tests\n"
        "test foo::bar ... ok\n"
        "test foo::baz ... FAILED\n"
        "test qux ... FAILED\n"
    )
    result = _run(["--rust-failures", "--suite", "rust_workspace"], stdin=log)
    assert result.returncode == 0
    entries = json.loads(result.stdout)
    names = sorted(e["test_id"] for e in entries)
    assert names == ["foo::baz", "qux"]
    assert all(e["test_id_kind"] == "rust" for e in entries)
    assert all(e["suite"] == "rust_workspace" for e in entries)


def test_rust_failures_classifies_panic():
    log = "test t ... FAILED\nthread 'main' panicked at src/x.rs:1\n"
    result = _run(["--rust-failures", "--suite", "rust_rt", "--failure-kind", "assertion"], stdin=log)
    entries = json.loads(result.stdout)
    assert entries[0]["failure_kind"] == "panic"


def test_rust_failures_empty_log_is_empty_array():
    result = _run(["--rust-failures", "--suite", "aot"], stdin="all passed\n")
    assert result.returncode == 0
    assert json.loads(result.stdout) == []


def test_rust_failures_missing_file_is_empty_array(tmp_path):
    missing = tmp_path / "nope.log"
    result = _run(["--rust-failures", "--suite", "aot", str(missing)])
    assert result.returncode == 0
    assert json.loads(result.stdout) == []


def test_rust_failures_requires_suite():
    result = _run(["--rust-failures"], stdin="test t ... FAILED\n")
    assert result.returncode == 2
