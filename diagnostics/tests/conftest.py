"""Shared fixtures for diagnostics/tests/test_parse_test_json*.py.

Provides the subprocess-invocation and fixture-summary-object builders each
split test module consumes, so the CLI-invocation shape and the base runner-
JSON summary shape stay single-sourced across the split test files.
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path
from typing import Callable

import pytest

HELPER = Path(__file__).resolve().parent.parent / "parse_test_json.py"


@pytest.fixture
def run_ptj() -> Callable[..., subprocess.CompletedProcess]:
    """Invoke `parse_test_json.py` as a subprocess; returns the CompletedProcess."""

    def _run(args, stdin=None):
        return subprocess.run(
            [sys.executable, str(HELPER), *args],
            input=stdin,
            capture_output=True,
            text=True,
        )

    return _run


@pytest.fixture
def valid_summary() -> Callable[..., dict]:
    """Build a runner-JSON summary object; **overrides replace default fields."""

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

    return _valid_summary


@pytest.fixture
def summary_with_failure(valid_summary) -> Callable[[str], dict]:
    """Build a one-failure summary object carrying the given failure message."""

    def _summary_with_failure(message):
        return valid_summary(
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

    return _summary_with_failure
