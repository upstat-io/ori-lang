"""Tests for diagnostics/parse_test_json.py --failures-json mode.

Sibling `test_parse_test_json.py` covers --counts / --summary-line /
--fail-lines; `test_parse_test_json_rust_failures.py` covers
--rust-failures.

Covers:
  - adversarial failure messages (tab / newline / quote / unicode / control)
    round-trip byte-identically through --failures-json (json.dumps) and back
    through json.loads
  - source-path attribution: every failure carries its .ori path
  - per-failure leak_free source signal
"""

import json
import sys
from pathlib import Path

import pytest

# Inline-import the module so failure-entry round-trip asserts at the Python
# boundary (byte-identity), in addition to the subprocess CLI tests.
_HELPER_DIR = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(_HELPER_DIR))
import parse_test_json  # noqa: E402

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


@pytest.mark.parametrize("message", ADVERSARIAL_MESSAGES)
def test_failures_json_roundtrips_byte_identically(run_ptj, summary_with_failure, message):
    obj = summary_with_failure(message)
    result = run_ptj(["--failures-json", "--suite", "ori_interp"], stdin=json.dumps(obj))
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
def test_failure_entries_helper_preserves_message(summary_with_failure, message):
    obj = summary_with_failure(message)
    entries = parse_test_json._failure_entries(obj, "ori_interp")
    assert len(entries) == 1
    assert entries[0]["error_message"] == message


def test_failures_json_emits_jq_valid_array_for_empty_failures(run_ptj, valid_summary):
    obj = valid_summary(failed=0, files=[])
    result = run_ptj(["--failures-json", "--suite", "ori_llvm"], stdin=json.dumps(obj))
    assert result.returncode == 0
    assert json.loads(result.stdout) == []


def test_failures_json_requires_suite(run_ptj, valid_summary):
    obj = valid_summary()
    result = run_ptj(["--failures-json"], stdin=json.dumps(obj))
    assert result.returncode == 2


def test_failures_json_captures_file_errors_and_llvm_compile_fail(run_ptj, valid_summary):
    obj = valid_summary(
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
    result = run_ptj(["--failures-json", "--suite", "ori_llvm"], stdin=json.dumps(obj))
    assert result.returncode == 0
    entries = json.loads(result.stdout)
    kinds = sorted(e["failure_kind"] for e in entries)
    assert kinds == ["file_error", "llvm_compile_fail"]


# --- source-path attribution: every failure carries its .ori path ---

def test_failure_entries_carry_source_path_for_every_kind(valid_summary):
    obj = valid_summary(
        failed=2,
        error_files=1,
        files=[{
            "path": "tests/spec/traits/iterator/map.ori",
            "results": [
                {
                    "name": "map_doubles_each_element",
                    "targets": ["map"],
                    "outcome": {"Failed": "assert_eq mismatch"},
                    "duration_ns": 1,
                },
                {
                    "name": "map_on_empty_list",
                    "targets": ["map"],
                    "outcome": {"LlvmCompileFail": "codegen exploded"},
                    "duration_ns": 0,
                },
            ],
            "passed": 0,
            "failed": 2,
            "skipped": 0,
            "skipped_unchanged": 0,
            "llvm_compile_fail": 1,
            "duration_ns": 1,
            "errors": ["type error: cannot unify"],
            "llvm_compile_error": True,
        }],
    )
    entries = parse_test_json._failure_entries(obj, "ori_llvm")
    # all three failure kinds present, each attributed to the owning .ori path
    assert len(entries) == 3
    assert {e["failure_kind"] for e in entries} == {
        "assertion_failure", "llvm_compile_fail", "file_error",
    }
    for entry in entries:
        assert entry["source_path"] == "tests/spec/traits/iterator/map.ori"


def test_source_path_empty_when_runner_omits_path(valid_summary):
    # fail-closed: a file_summary with no path attributes to "" (downstream
    # records parity=unknown), never silently dropped.
    obj = valid_summary(
        failed=1,
        files=[{
            "results": [{
                "name": "orphan_test",
                "targets": [],
                "outcome": {"Failed": "boom"},
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
    entries = parse_test_json._failure_entries(obj, "ori_interp")
    assert len(entries) == 1
    assert entries[0]["source_path"] == ""


# --- per-failure leak_free source signal ---

def test_leak_positive_classifies_rc_leak_failures(valid_summary):
    # The runtime emits "N RC allocation(s) not freed (memory leak)" under
    # ORI_CHECK_LEAKS=1; the spec runner wraps it as "ARC leak: ...". Each form
    # marks the failure leak_positive so the per-node leak_free leg has a signal.
    obj = valid_summary(
        failed=3,
        files=[{
            "path": "tests/spec/collections/cow/push.ori",
            "results": [
                {
                    "name": "push_leaks_under_check",
                    "targets": ["push"],
                    "outcome": {"Failed": "2 RC allocation(s) not freed (memory leak)"},
                    "duration_ns": 1,
                },
                {
                    "name": "push_arc_leak_variant",
                    "targets": ["push"],
                    "outcome": {"Failed": "ARC leak: 1 allocation(s) not freed"},
                    "duration_ns": 1,
                },
                {
                    "name": "push_plain_assertion",
                    "targets": ["push"],
                    "outcome": {"Failed": "assert_eq mismatch: 3 != 4"},
                    "duration_ns": 1,
                },
            ],
            "passed": 0,
            "failed": 3,
            "skipped": 0,
            "skipped_unchanged": 0,
            "llvm_compile_fail": 0,
            "duration_ns": 1,
            "errors": [],
            "llvm_compile_error": False,
        }],
    )
    entries = parse_test_json._failure_entries(obj, "ori_llvm")
    by_name = {e["test_id"]: e for e in entries}
    assert by_name["push_leaks_under_check"]["leak_positive"] is True
    assert by_name["push_arc_leak_variant"]["leak_positive"] is True
    # a non-leak assertion failure is leak_positive False (NOT unknown — the
    # per-node verdict fail-closes leak_free to unknown for nodes with no signal)
    assert by_name["push_plain_assertion"]["leak_positive"] is False
    # the leak signal travels with the .ori source path
    assert all(
        e["source_path"] == "tests/spec/collections/cow/push.ori" for e in entries
    )
