"""Tests for diagnostics/parse_test_json.py — counts / parse-error-contract /
summary-line / fail-lines modes.

Sibling files cover the remaining modes:
  - test_parse_test_json_failures.py — --failures-json (adversarial byte
    round-trip, source-path attribution, leak_positive classification)
  - test_parse_test_json_rust_failures.py — --rust-failures (cargo/nextest
    log scraping)

Covers the contract the bash harness depends on:
  - valid JSON object -> correct KEY=value counts
  - invalid / non-object / count-missing JSON -> non-zero exit (NEVER silent
    default-zero, the failure mode the JSON consumption fixes)
  - the reconstructed --summary-line matches print_summary_stats shape
"""

import json


# --- counts mode ---

def test_counts_valid_json_emits_correct_keys(run_ptj, valid_summary):
    obj = valid_summary(passed=10, failed=2, skipped=1, llvm_compile_fail=3)
    result = run_ptj(["--counts"], stdin=json.dumps(obj))
    assert result.returncode == 0
    lines = dict(line.split("=", 1) for line in result.stdout.splitlines())
    assert lines == {
        "PASSED": "10",
        "FAILED": "2",
        "SKIPPED": "1",
        "LCFAIL": "3",
        "CRASHED": "0",
    }


def test_counts_from_file_argument(run_ptj, valid_summary, tmp_path):
    path = tmp_path / "results.json"
    path.write_text(json.dumps(valid_summary(passed=7)), encoding="utf-8")
    result = run_ptj(["--counts", str(path)])
    assert result.returncode == 0
    assert "PASSED=7" in result.stdout.splitlines()


# --- parse-error contract: NON-ZERO, never silent default-zero ---

def test_invalid_json_exits_nonzero(run_ptj):
    result = run_ptj(["--counts"], stdin="this is not json {")
    assert result.returncode != 0
    assert "PASSED=" not in result.stdout
    assert "invalid runner JSON" in result.stderr


def test_partial_then_garbage_json_exits_nonzero(run_ptj):
    # SIGPIPE / stderr-leak shape: a valid prefix followed by garbage.
    result = run_ptj(["--counts"], stdin='{"passed": 3, "failed"')
    assert result.returncode != 0
    assert "PASSED=" not in result.stdout


def test_non_object_json_exits_nonzero(run_ptj):
    result = run_ptj(["--counts"], stdin="[1, 2, 3]")
    assert result.returncode != 0
    assert "expected a JSON object" in result.stderr


def test_object_missing_count_field_exits_nonzero(run_ptj, valid_summary):
    obj = valid_summary()
    del obj["passed"]
    result = run_ptj(["--counts"], stdin=json.dumps(obj))
    assert result.returncode != 0
    assert "missing required aggregate count field" in result.stderr


def test_count_field_not_integer_exits_nonzero(run_ptj, valid_summary):
    obj = valid_summary()
    obj["passed"] = "ten"
    result = run_ptj(["--counts"], stdin=json.dumps(obj))
    assert result.returncode != 0
    assert "not an integer" in result.stderr


def test_empty_input_exits_nonzero(run_ptj):
    result = run_ptj(["--counts"], stdin="")
    assert result.returncode != 0


# --- summary-line reconstruction ---

def test_summary_line_basic(run_ptj, valid_summary):
    obj = valid_summary(passed=10, failed=2, skipped=1)
    result = run_ptj(["--summary-line"], stdin=json.dumps(obj))
    assert result.returncode == 0
    assert result.stdout.strip() == "10 passed, 2 failed, 1 skipped"


def test_summary_line_appends_extra_segments(run_ptj, valid_summary):
    obj = valid_summary(
        passed=5, failed=0, skipped=0,
        skipped_unchanged=3, llvm_compile_fail=2, error_files=1,
    )
    result = run_ptj(["--summary-line"], stdin=json.dumps(obj))
    assert result.returncode == 0
    assert result.stdout.strip() == (
        "5 passed, 0 failed, 0 skipped, 3 skipped (unchanged), "
        "2 llvm compile fail, 1 files with errors"
    )


def test_summary_line_invalid_json_exits_nonzero(run_ptj):
    result = run_ptj(["--summary-line"], stdin="garbage")
    assert result.returncode != 0


# --- fail-lines reconstruction ---

def test_fail_lines_renders_failed_test(run_ptj, summary_with_failure):
    obj = summary_with_failure("assertion failed: 1 == 2")
    result = run_ptj(["--fail-lines"], stdin=json.dumps(obj))
    assert result.returncode == 0
    assert "tests/spec/adversarial.ori" in result.stdout
    assert "FAIL: adversarial_test - assertion failed: 1 == 2" in result.stdout


def test_fail_lines_renders_file_error_and_llvm_compile_fail(run_ptj, valid_summary):
    obj = valid_summary(
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
    result = run_ptj(["--fail-lines"], stdin=json.dumps(obj))
    assert result.returncode == 0
    assert "LLVM COMPILE FAIL: blocked - codegen exploded" in result.stdout
    assert "ERROR: type error: cannot unify" in result.stdout


def test_fail_lines_empty_for_all_pass(run_ptj, valid_summary):
    obj = valid_summary(failed=0, files=[])
    result = run_ptj(["--fail-lines"], stdin=json.dumps(obj))
    assert result.returncode == 0
    assert result.stdout.strip() == ""


def test_fail_lines_prints_path_once_per_file_across_multiple_files(run_ptj, valid_summary):
    """Regression: _emit_fail_lines walks files[] via the shared
    _iter_file_failures generator and tracks the "printed this file's path
    already" boundary by object identity (id(file_summary)) rather than a
    per-file boolean reset in an outer loop. Two files, each with 2 distinct
    failure lines, must each print their path exactly once (before their
    first failure line), never per-line and never merged across files."""
    obj = valid_summary(
        failed=4,
        files=[
            {
                "path": "tests/spec/first.ori",
                "results": [
                    {"name": "first_a", "targets": [], "duration_ns": 0,
                     "outcome": {"Failed": "boom a"}},
                    {"name": "first_b", "targets": [], "duration_ns": 0,
                     "outcome": {"Failed": "boom b"}},
                ],
                "passed": 0, "failed": 2, "skipped": 0, "skipped_unchanged": 0,
                "llvm_compile_fail": 0, "duration_ns": 0, "errors": [],
            },
            {
                "path": "tests/spec/second.ori",
                "results": [
                    {"name": "second_a", "targets": [], "duration_ns": 0,
                     "outcome": {"Failed": "boom c"}},
                    {"name": "second_b", "targets": [], "duration_ns": 0,
                     "outcome": {"Failed": "boom d"}},
                ],
                "passed": 0, "failed": 2, "skipped": 0, "skipped_unchanged": 0,
                "llvm_compile_fail": 0, "duration_ns": 0, "errors": [],
            },
        ],
    )
    result = run_ptj(["--fail-lines"], stdin=json.dumps(obj))
    assert result.returncode == 0
    lines = result.stdout.splitlines()
    assert lines.count("tests/spec/first.ori") == 1
    assert lines.count("tests/spec/second.ori") == 1
    assert "  FAIL: first_a - boom a" in lines
    assert "  FAIL: first_b - boom b" in lines
    assert "  FAIL: second_a - boom c" in lines
    assert "  FAIL: second_b - boom d" in lines
    # Path line for a file precedes its own failure lines, and the second
    # file's path does not appear until after the first file's lines end.
    first_path_idx = lines.index("tests/spec/first.ori")
    second_path_idx = lines.index("tests/spec/second.ori")
    assert first_path_idx < lines.index("  FAIL: first_a - boom a")
    assert second_path_idx > lines.index("  FAIL: first_b - boom b")
    assert second_path_idx < lines.index("  FAIL: second_a - boom c")
