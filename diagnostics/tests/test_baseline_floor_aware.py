"""TDD matrix for floor-aware `state.sh baseline compare`.

The per-section test-baseline regression gate (`baseline_compare()` in
diagnostics/state.sh) must EXCLUDE known aims-burden AOT floor cells
(diagnostics/baseline_failing_ids.txt) from `newly_failing` / `newly_fixed`, so
a known floor cell never reads as a section-introduced regression.

These tests drive the REAL `state.sh baseline compare --json` against synthetic
state/baseline/floor fixtures via the `ORI_STATE_FILE` / `ORI_BASELINES_FILE` /
`ORI_BASELINE_FLOOR_FILE` env overrides. Fail-first on the floor-blind code (the
3 semantic pins): SC-1 (floor cell flagged as a regression) + SC-3 (floor fix
spuriously credited) + SC-6 (production-default floor path unread). SC-2 + SC-4
are the negative pins — a genuine non-floor regression / non-floor fix MUST
still be caught / credited; SC-5 is the missing-floor legacy-degradation guard.
Verified `3 failed, 3 passed` against the unfixed `state.sh`.
"""
from __future__ import annotations

import json
import subprocess
from pathlib import Path

import pytest

_STATE_SH = Path(__file__).resolve().parents[1] / "state.sh"


def _write_state(path: Path, failing_ids: list[str]) -> None:
    """Write a synthetic known-state.json whose test_suite.failures carry the
    given test ids (status known-failing so require_measured_test_suite passes)."""
    path.write_text(json.dumps({
        "head_sha": "deadbee",
        "test_suite": {
            "status": "known-failing",
            "last_run_sha": "deadbee",
            "known_failing_count": len(failing_ids),
            "totals": {"passed": 100, "failed": len(failing_ids), "skipped": 0},
            "failures": [{"test_id": t} for t in failing_ids],
        },
        "clippy": {"status": "clean", "warnings": 0, "errors": 0},
        "test_dispositions": {"totals": {"total": 0, "untracked": 0}},
    }))


def _write_baseline(path: Path, key: str, failing_ids: list[str]) -> None:
    path.write_text(json.dumps({
        "schema_version": 1,
        "baselines": {
            key: {
                "key": key,
                "captured_at": "2026-06-16T00:00:00Z",
                "captured_at_sha": "deadbee",
                "captured_by": "test",
                "test_suite": {
                    "status": "known-failing",
                    "known_failing_count": len(failing_ids),
                    "totals": {"passed": 100, "failed": len(failing_ids), "skipped": 0},
                    "failures": sorted(set(failing_ids)),
                },
                "clippy": {"status": "clean", "warnings": 0, "errors": 0},
                "test_dispositions": {"total": 0, "untracked": 0},
            }
        },
    }))


def _write_degraded_baseline(path: Path, key: str) -> None:
    path.write_text(json.dumps({
        "schema_version": 1,
        "baselines": {
            key: {
                "key": key,
                "captured_at": "2026-06-16T00:00:00Z",
                "captured_at_sha": "deadbee",
                "captured_by": "test",
                "test_suite": {
                    "status": "known-failing",
                    "known_failing_count": 0,
                    "totals": {"passed": 0, "failed": 6, "skipped": 0},
                    "failures": [],
                },
                "clippy": {"status": "clean", "warnings": 0, "errors": 0},
                "test_dispositions": {"total": 0, "untracked": 0},
            }
        },
    }))


def _compare(tmp_path: Path, *, baseline: list[str], current: list[str],
             floor: list[str], key: str = "sec") -> dict:
    """Run the real `state.sh baseline compare --key <key> --json` against
    synthetic fixtures; return the parsed JSON report."""
    state_f = tmp_path / "known-state.json"
    base_f = tmp_path / "baselines.json"
    floor_f = tmp_path / "floor.txt"
    _write_state(state_f, current)
    _write_baseline(base_f, key, baseline)
    floor_f.write_text("\n".join(floor) + ("\n" if floor else ""))
    env = {
        "ORI_STATE_FILE": str(state_f),
        "ORI_BASELINES_FILE": str(base_f),
        "ORI_BASELINE_FLOOR_FILE": str(floor_f),
        "PATH": __import__("os").environ.get("PATH", ""),
    }
    r = subprocess.run(
        ["bash", str(_STATE_SH), "baseline", "compare", "--key", key, "--json"],
        capture_output=True, text=True, env=env, timeout=60,
    )
    # exit 0 (clean) or 5 (regression) both emit the JSON report on stdout.
    assert r.stdout.strip(), f"no JSON report (exit {r.returncode}): {r.stderr[-400:]}"
    return json.loads(r.stdout)


_FLOOR_CELL = "aims_interactions::test_h12_rc_merge_preserves_rc_ops"


def test_sc1_known_floor_cell_is_not_a_regression(tmp_path):
    """SC-1 (fail-first): the only delta vs baseline is a known floor cell →
    the gate MUST pass (floor-aware newly_failing is empty)."""
    report = _compare(
        tmp_path,
        baseline=["other::ok_test"],
        current=["other::ok_test", _FLOOR_CELL],
        floor=[_FLOOR_CELL],
    )
    assert report["newly_failing"] == [], report
    assert report["gate_pass"] is True, report
    assert report["regression"] is False, report


def test_sc2_genuine_non_floor_regression_still_caught(tmp_path):
    """SC-2 (negative pin): a NEW non-floor failure MUST still be flagged —
    floor-subtraction does not weaken real-regression detection."""
    new_fail = "typeck::test_brand_new_real_regression"
    report = _compare(
        tmp_path,
        baseline=["other::ok_test"],
        current=["other::ok_test", new_fail],
        floor=[_FLOOR_CELL],  # floor does NOT contain new_fail
    )
    assert new_fail in report["newly_failing"], report
    assert report["gate_pass"] is False, report
    assert report["regression"] is True, report


def test_sc3_floor_fix_not_credited_as_section_improvement(tmp_path):
    """SC-3 (fail-first): a floor cell that flips baseline-failing →
    current-passing MUST NOT be credited in newly_fixed (aims-burden owns it)."""
    report = _compare(
        tmp_path,
        baseline=["other::ok_test", _FLOOR_CELL],
        current=["other::ok_test"],
        floor=[_FLOOR_CELL],
    )
    assert _FLOOR_CELL not in report["newly_fixed"], report


def test_sc4_non_floor_fix_still_credited(tmp_path):
    """SC-4 (negative pin): a NON-floor failure that flips to passing IS still
    credited in newly_fixed — floor-subtraction is scoped to floor cells only."""
    fixed = "eval::test_real_fix"
    report = _compare(
        tmp_path,
        baseline=["other::ok_test", fixed],
        current=["other::ok_test"],
        floor=[_FLOOR_CELL],  # floor does NOT contain `fixed`
    )
    assert fixed in report["newly_fixed"], report


def test_sc5_missing_floor_file_degrades_to_legacy(tmp_path):
    """SC-5: a missing floor file degrades to legacy (empty-floor) behavior —
    every current-minus-baseline failure is a regression, never errors."""
    state_f = tmp_path / "known-state.json"
    base_f = tmp_path / "baselines.json"
    _write_state(state_f, ["other::ok_test", _FLOOR_CELL])
    _write_baseline(base_f, "sec", ["other::ok_test"])
    env = {
        "ORI_STATE_FILE": str(state_f),
        "ORI_BASELINES_FILE": str(base_f),
        "ORI_BASELINE_FLOOR_FILE": str(tmp_path / "does-not-exist.txt"),
        "PATH": __import__("os").environ.get("PATH", ""),
    }
    r = subprocess.run(
        ["bash", str(_STATE_SH), "baseline", "compare", "--key", "sec", "--json"],
        capture_output=True, text=True, env=env, timeout=60,
    )
    report = json.loads(r.stdout)
    # No floor → the floor cell is a (legacy) regression; no crash.
    assert _FLOOR_CELL in report["newly_failing"], report
    assert report["gate_pass"] is False, report


_REAL_FLOOR = Path(__file__).resolve().parents[1] / "baseline_failing_ids.txt"


def test_sc6_production_default_floor_path_excludes_known_cell(tmp_path):
    """SC-6 (fail-first; production-default pin): with NO
    ORI_BASELINE_FLOOR_FILE override, baseline_compare must read the PRODUCTION
    default floor (diagnostics/baseline_failing_ids.txt — the path the live
    callers verify-and-complete-v7 / aot-guardrail.sh use) and exclude a real
    floor cell from newly_failing. Pins the default-config path, not just the
    test-injected seam."""
    assert _FLOOR_CELL in _REAL_FLOOR.read_text().splitlines(), (
        f"{_FLOOR_CELL} must be a member of the real floor {_REAL_FLOOR}"
    )
    state_f = tmp_path / "known-state.json"
    base_f = tmp_path / "baselines.json"
    _write_state(state_f, ["other::ok_test", _FLOOR_CELL])
    _write_baseline(base_f, "sec", ["other::ok_test"])
    env = {
        "ORI_STATE_FILE": str(state_f),
        "ORI_BASELINES_FILE": str(base_f),
        # NO ORI_BASELINE_FLOOR_FILE — exercise the production default floor path.
        "PATH": __import__("os").environ.get("PATH", ""),
    }
    r = subprocess.run(
        ["bash", str(_STATE_SH), "baseline", "compare", "--key", "sec", "--json"],
        capture_output=True, text=True, env=env, timeout=60,
    )
    report = json.loads(r.stdout)
    assert report["newly_failing"] == [], report
    assert report["gate_pass"] is True, report


def test_degraded_baseline_empty_failure_ids_soft_passes(tmp_path):
    """A baseline with failed>0 but no failure ids cannot produce a set diff.
    The comparator returns a degraded soft-pass instead of flagging every current
    failure as section-introduced."""
    state_f = tmp_path / "known-state.json"
    base_f = tmp_path / "baselines.json"
    floor_f = tmp_path / "floor.txt"
    _write_state(state_f, ["real::failure"])
    _write_degraded_baseline(base_f, "sec")
    floor_f.write_text("")
    env = {
        "ORI_STATE_FILE": str(state_f),
        "ORI_BASELINES_FILE": str(base_f),
        "ORI_BASELINE_FLOOR_FILE": str(floor_f),
        "PATH": __import__("os").environ.get("PATH", ""),
    }
    r = subprocess.run(
        ["bash", str(_STATE_SH), "baseline", "compare", "--key", "sec", "--json"],
        capture_output=True, text=True, env=env, timeout=60,
    )
    report = json.loads(r.stdout)
    assert r.returncode == 6, report
    assert report["baseline_degraded"] is True, report
    assert report["gate_pass"] is True, report
    assert report["newly_failing"] == [], report
