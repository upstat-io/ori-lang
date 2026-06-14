"""Tests for diagnostics/parse_test_json_roundtrip_check.py — the adversarial
failure-message round-trip-equality check (w-38896e3b).

The semantic pin: an adversarial message (tab/newline/quote/unicode) survives
json.dumps -> file -> json.load byte-for-byte. The negative pins: a non-matching
expected message reports MISMATCH (exit 4), and invalid JSON reports parse-error
(exit 3) rather than silent default-pass. Positive + negative pairing per
tests.md §Matrix Clamping.
"""

import json
import subprocess
import sys
from pathlib import Path

import pytest

HELPER = Path(__file__).resolve().parent.parent / "parse_test_json_roundtrip_check.py"

# Inline-import so collect_messages asserts at the Python level too.
sys.path.insert(0, str(HELPER.parent))
import parse_test_json_roundtrip_check as rtc  # noqa: E402

# The full adversarial payload the section names: tab, newline, quote, unicode.
ADV_FULL = 'tab\there\nnewline"quoteéunicode'

# Per-char matrix — each adversarial class must survive independently (squeeze).
ADV_CASES = {
    "tab": "before\tafter",
    "newline": "before\nafter",
    "carriage_return": "before\rafter",
    "double_quote": 'before"after',
    "backslash": "before\\after",
    "unicode": "café☃\U0001f600",
    "control_null_ish": "before\x01\x02after",
    "all_combined": ADV_FULL,
}


def _harness_summary(message):
    return {"suites": [{"name": "adv", "status": "failed",
                        "failures": [{"message": message}]}]}


def _runner_summary(message):
    # Runner --format json shape: files[].results[].outcome = {"Failed": msg}.
    return {
        "passed": 0, "failed": 1, "skipped": 0, "llvm_compile_fail": 0,
        "files": [{
            "path": "tests/spec/adv.ori",
            "results": [{"name": "adv_test", "targets": [],
                         "outcome": {"Failed": message}, "duration_ns": 0}],
            "errors": [],
        }],
    }


def _write(tmp_path, obj):
    p = tmp_path / "summary.json"
    p.write_text(json.dumps(obj, ensure_ascii=False), encoding="utf-8")
    return p


def _run(path, expect=None):
    argv = [str(path)]
    if expect is not None:
        argv += ["--expect-message", expect]
    return subprocess.run([sys.executable, str(HELPER), *argv],
                          capture_output=True, text=True)


# --- semantic pins: each adversarial class survives byte-for-byte ---

@pytest.mark.parametrize("name,msg", ADV_CASES.items())
def test_harness_shape_roundtrip_survives(tmp_path, name, msg):
    path = _write(tmp_path, _harness_summary(msg))
    result = _run(path, expect=msg)
    assert result.returncode == rtc.EXIT_OK, \
        f"adversarial class {name!r} did not survive: {result.stderr}"


@pytest.mark.parametrize("name,msg", ADV_CASES.items())
def test_runner_shape_roundtrip_survives(tmp_path, name, msg):
    path = _write(tmp_path, _runner_summary(msg))
    result = _run(path, expect=msg)
    assert result.returncode == rtc.EXIT_OK, \
        f"runner-shape adversarial class {name!r} did not survive: {result.stderr}"


def test_self_verifying_matrix_count():
    # Proves the per-char matrix visits every named class (Zig self-verify).
    assert len(ADV_CASES) == 8


# --- negative pins: mismatch + parse-error are not silent passes ---

def test_mismatch_expected_message_absent(tmp_path):
    path = _write(tmp_path, _harness_summary(ADV_FULL))
    result = _run(path, expect="a different message entirely")
    assert result.returncode == rtc.EXIT_MISMATCH


def test_mismatch_altered_byte(tmp_path):
    # The stored message differs from expected by ONE byte — must not pass.
    path = _write(tmp_path, _harness_summary("before\tafter"))
    result = _run(path, expect="before after")  # tab replaced by space
    assert result.returncode == rtc.EXIT_MISMATCH


def test_invalid_json_is_parse_error(tmp_path):
    p = tmp_path / "bad.json"
    p.write_text("not-json{", encoding="utf-8")
    result = _run(p, expect=ADV_FULL)
    assert result.returncode == rtc.EXIT_PARSE_ERROR


def test_non_object_json_is_parse_error(tmp_path):
    p = tmp_path / "list.json"
    p.write_text("[1, 2, 3]", encoding="utf-8")
    result = _run(p, expect=ADV_FULL)
    assert result.returncode == rtc.EXIT_PARSE_ERROR


def test_no_expect_message_roundtrip_parse_only(tmp_path):
    # Without --expect-message, valid round-trip parse alone is the assertion.
    path = _write(tmp_path, _harness_summary(ADV_FULL))
    result = _run(path)
    assert result.returncode == rtc.EXIT_OK


def test_no_expect_message_invalid_json_still_fails(tmp_path):
    p = tmp_path / "bad.json"
    p.write_text("}{", encoding="utf-8")
    result = _run(p)
    assert result.returncode == rtc.EXIT_PARSE_ERROR


# --- unit: collect_messages spans both shapes ---

def test_collect_messages_harness_and_runner_shapes():
    harness = rtc.collect_messages(_harness_summary("h-msg"))
    runner = rtc.collect_messages(_runner_summary("r-msg"))
    combined = rtc.collect_messages({
        **_runner_summary("r-msg"),
        "suites": [{"name": "s", "failures": [{"message": "h-msg"}]}],
    })
    assert harness == ["h-msg"]
    assert runner == ["r-msg"]
    assert sorted(combined) == ["h-msg", "r-msg"]
