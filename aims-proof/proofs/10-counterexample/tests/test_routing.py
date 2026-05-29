"""Test matrix per §10.1 checkbox 10 + §10.2 close-out item 5.

Asserts each of the 7 §10 close-out `exit_reason` keys has BOTH:
  (a) a positive test row driving a synthetic shape that produces the exit_reason
  (b) a route-on-emit assertion verifying the runner emits the correct
      exit_reason for that shape.

The 7 exit_reasons (per the counterexample search
§10.2 close-out item 5):
  1. counterexample_search_adequate (all rows UNSAT, fixtures green) → exit 0
  2. counterexample_witness_found (>=1 SAT row, recovery cycle live) → exit 2
  3. counterexample_proof_gap_unprovable (SAT unrecovered) → exit 2
  4. counterexample_reformulation_required (SAT recovered via reformulation) → exit 2
  5. encoder_invalid_halt (encoder-validation fixture failed) → exit 2
  6. smt_timeout_inconclusive (>=1 TIMEOUT_EXHAUSTED) → exit 2
  7. counterexample_infrastructure_failed (solver/scaffold/runner fail) → exit 3

Partial coverage (only PASS + one FAIL) is flagged as
INVERTED-TDD:positive-test-modification per impl-hygiene.md §Finding Categories.
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[4]
RUNNER = REPO_ROOT / "aims-proof" / "scripts" / "run-section-10-counterexample.sh"
FIXTURE_FILE = REPO_ROOT / "aims-proof" / "proofs" / "10-counterexample" / "fixtures" / "encoder-validation.smt2"


def _solver_available() -> str | None:
    for solver in ("z3", "cvc5"):
        if shutil.which(solver):
            return solver
    return None


def _make_stub_solver(tmpdir: Path, name: str, script: str) -> Path:
    """Write a shell stub that mimics a solver's stdout for the runner.

    The stub is invoked as `<stub> <smt2_path>`; it echoes deterministic
    sat/unsat/unknown lines per the script body. The §10 runner parses
    the FIRST sat/unsat token from solver stdout (per run_solver_bounded_retry).
    """
    path = tmpdir / name
    path.write_text(script)
    path.chmod(0o755)
    return path


def _invoke_runner(
    *,
    tmpdir: Path,
    solver_stub: Path | None = None,
    extra_env: dict[str, str] | None = None,
    extra_args: list[str] | None = None,
) -> subprocess.CompletedProcess[str]:
    """Invoke the §10 runner with a synthetic-fixture-corpus tmpdir.

    The tmpdir mirrors the aims-proof/ layout enough for the runner's cwd
    anchor to work: scripts/run-section-10-counterexample.sh + proofs/10-
    counterexample/fixtures/encoder-validation.smt2 + (optionally) shape
    fixtures in proofs/10-counterexample/.
    """
    env = dict(os.environ)
    if extra_env:
        env.update(extra_env)
    args = [str(tmpdir / "scripts" / "run-section-10-counterexample.sh")]
    if solver_stub is not None:
        args += ["--solver", str(solver_stub)]
    if extra_args:
        args += extra_args
    return subprocess.run(
        args,
        env=env,
        capture_output=True,
        text=True,
        timeout=120,
    )


def _scaffold_tmpdir(tmpdir: Path, shapes: dict[str, str] | None = None) -> None:
    """Mirror aims-proof/ layout under tmpdir.

    - scripts/run-section-10-counterexample.sh — symlink to the real runner.
    - proofs/10-counterexample/fixtures/encoder-validation.smt2 — symlink to
      the real fixture (the real fixture is content-deterministic; the
      stub solver responds to the file's existence + ordering).
    - proofs/10-counterexample/<shape>.smt2 — written per shapes dict.
    - test-results/ — empty dir for runner output.
    """
    (tmpdir / "scripts").mkdir()
    (tmpdir / "scripts" / "run-section-10-counterexample.sh").symlink_to(RUNNER)
    (tmpdir / "proofs" / "10-counterexample" / "fixtures").mkdir(parents=True)
    (tmpdir / "proofs" / "10-counterexample" / "fixtures" / "encoder-validation.smt2").symlink_to(FIXTURE_FILE)
    (tmpdir / "proofs" / "10-counterexample" / "fixtures" / "encoder-validation.json").symlink_to(
        FIXTURE_FILE.with_suffix(".json")
    )
    (tmpdir / "test-results").mkdir()
    for name, content in (shapes or {}).items():
        (tmpdir / "proofs" / "10-counterexample" / f"{name}.smt2").write_text(content)


def _read_results(tmpdir: Path) -> dict:
    path = tmpdir / "proofs" / "10-counterexample" / "results.json"
    return json.loads(path.read_text())


# ---------------------------------------------------------------------------
# Stub solver scripts — each mimics one solver-verdict pattern.
# Each `(check-sat)` in the input SMT file maps to one emitted sat/unsat
# line; the stub doesn't actually parse the SMT — it counts (check-sat)
# occurrences and emits one verdict per occurrence.
# ---------------------------------------------------------------------------

def _stub_emit_per_check_sat(verdicts: list[str]) -> str:
    """Build a stub-solver bash script that emits `verdicts[i]` for the
    i-th `(check-sat)` call in the input file.
    """
    verdict_csv = " ".join(verdicts)
    return f"""#!/usr/bin/env bash
# Stub solver: emits one fixed verdict per (check-sat) call in input.
INPUT="$1"
VERDICTS=({verdict_csv})
i=0
while IFS= read -r line; do
    if [[ "$line" == *"(check-sat)"* ]]; then
        if [[ "$i" -lt "${{#VERDICTS[@]}}" ]]; then
            echo "${{VERDICTS[$i]}}"
            i=$((i + 1))
        fi
    fi
done < "$INPUT"
"""


def _stub_emit_fixed_verdict_then_per_shape(fixture_verdicts: list[str], shape_verdict: str) -> str:
    """Stub: fixture file emits `fixture_verdicts` (2 entries: known-SAT,
    known-UNSAT); every other file emits `shape_verdict` once.
    """
    fv_csv = " ".join(fixture_verdicts)
    return f"""#!/usr/bin/env bash
INPUT="$1"
if [[ "$INPUT" == *"encoder-validation.smt2" ]]; then
    VERDICTS=({fv_csv})
    i=0
    while IFS= read -r line; do
        if [[ "$line" == *"(check-sat)"* ]]; then
            if [[ "$i" -lt "${{#VERDICTS[@]}}" ]]; then
                echo "${{VERDICTS[$i]}}"
                i=$((i + 1))
            fi
        fi
    done < "$INPUT"
else
    echo "{shape_verdict}"
fi
"""


def _stub_timeout_on_shape() -> str:
    """Stub: fixture passes (sat/unsat); shape files sleep beyond budget.

    The stub sleeps `999` seconds on shape files; the runner's `timeout`
    wrapper kills it at the per-attempt budget. With initial-timeout 1s +
    max-retries 0, the stub triggers TIMEOUT_EXHAUSTED on first attempt.
    """
    return """#!/usr/bin/env bash
INPUT="$1"
if [[ "$INPUT" == *"encoder-validation.smt2" ]]; then
    echo "sat"
    echo "unsat"
else
    sleep 999
fi
"""


# ---------------------------------------------------------------------------
# Test (1) — counterexample_search_adequate (all UNSAT, fixtures green)
# ---------------------------------------------------------------------------

def test_counterexample_search_adequate(tmp_path: Path) -> None:
    _scaffold_tmpdir(tmp_path, shapes={
        "shape-a": "(check-sat)\n",
        "shape-b": "(check-sat)\n",
    })
    stub = _make_stub_solver(
        tmp_path,
        "stub-solver.sh",
        _stub_emit_fixed_verdict_then_per_shape(["sat", "unsat"], "unsat"),
    )
    result = _invoke_runner(tmpdir=tmp_path, solver_stub=stub)
    assert result.returncode == 0, f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}"
    results = _read_results(tmp_path)
    assert results["exit_reason"] == "counterexample_search_adequate"
    assert all(row["smt_result"] in ("UNSAT", "SAT") for row in results["runs"])


# ---------------------------------------------------------------------------
# Test (2) — counterexample_witness_found (>=1 SAT, recovery cycle live)
# ---------------------------------------------------------------------------

def test_counterexample_witness_found(tmp_path: Path) -> None:
    _scaffold_tmpdir(tmp_path, shapes={
        "shape-a": "(check-sat)\n",
    })
    stub = _make_stub_solver(
        tmp_path,
        "stub-solver.sh",
        _stub_emit_fixed_verdict_then_per_shape(["sat", "unsat"], "sat"),
    )
    result = _invoke_runner(tmpdir=tmp_path, solver_stub=stub)
    assert result.returncode == 2, f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}"
    results = _read_results(tmp_path)
    assert results["exit_reason"] == "counterexample_witness_found"
    sat_rows = [row for row in results["runs"] if row["smt_result"] == "SAT" and row["shape_id"].startswith("shape-")]
    assert sat_rows, "expected >=1 SAT row on real shape"


# ---------------------------------------------------------------------------
# Test (3) — counterexample_proof_gap_unprovable (SAT unrecovered)
# ---------------------------------------------------------------------------

def test_counterexample_proof_gap_unprovable(tmp_path: Path) -> None:
    _scaffold_tmpdir(tmp_path, shapes={"shape-a": "(check-sat)\n"})
    stub = _make_stub_solver(
        tmp_path,
        "stub-solver.sh",
        _stub_emit_fixed_verdict_then_per_shape(["sat", "unsat"], "sat"),
    )
    result = _invoke_runner(
        tmpdir=tmp_path,
        solver_stub=stub,
        extra_env={"COUNTEREXAMPLE_SAT_CLASSIFICATION": "counterexample_proof_gap_unprovable"},
    )
    assert result.returncode == 2, f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}"
    results = _read_results(tmp_path)
    assert results["exit_reason"] == "counterexample_proof_gap_unprovable"


# ---------------------------------------------------------------------------
# Test (4) — counterexample_reformulation_required (SAT recovered via reform)
# ---------------------------------------------------------------------------

def test_counterexample_reformulation_required(tmp_path: Path) -> None:
    _scaffold_tmpdir(tmp_path, shapes={"shape-a": "(check-sat)\n"})
    stub = _make_stub_solver(
        tmp_path,
        "stub-solver.sh",
        _stub_emit_fixed_verdict_then_per_shape(["sat", "unsat"], "sat"),
    )
    result = _invoke_runner(
        tmpdir=tmp_path,
        solver_stub=stub,
        extra_env={"COUNTEREXAMPLE_SAT_CLASSIFICATION": "counterexample_reformulation_required"},
    )
    assert result.returncode == 2, f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}"
    results = _read_results(tmp_path)
    assert results["exit_reason"] == "counterexample_reformulation_required"


# ---------------------------------------------------------------------------
# Test (5) — encoder_invalid_halt (known-SAT non-SAT OR known-UNSAT non-UNSAT)
# ---------------------------------------------------------------------------

def test_encoder_invalid_halt_known_sat_failed(tmp_path: Path) -> None:
    _scaffold_tmpdir(tmp_path, shapes={"shape-a": "(check-sat)\n"})
    # Known-SAT returns unsat → vacuous-UNSAT hazard active.
    stub = _make_stub_solver(
        tmp_path,
        "stub-solver.sh",
        _stub_emit_fixed_verdict_then_per_shape(["unsat", "unsat"], "unsat"),
    )
    result = _invoke_runner(tmpdir=tmp_path, solver_stub=stub)
    assert result.returncode == 2, f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}"
    results = _read_results(tmp_path)
    assert results["exit_reason"] == "encoder_invalid_halt"
    # The known-SAT row should carry ENCODER_INVALID.
    known_sat = [r for r in results["runs"] if r["shape_id"] == "fixture-known-sat-vacuity-guard"]
    assert known_sat and known_sat[0]["smt_result"] == "ENCODER_INVALID"


def test_encoder_invalid_halt_known_unsat_failed(tmp_path: Path) -> None:
    _scaffold_tmpdir(tmp_path, shapes={"shape-a": "(check-sat)\n"})
    # Known-UNSAT returns sat → soundness sanity rejected a sound shape.
    stub = _make_stub_solver(
        tmp_path,
        "stub-solver.sh",
        _stub_emit_fixed_verdict_then_per_shape(["sat", "sat"], "unsat"),
    )
    result = _invoke_runner(tmpdir=tmp_path, solver_stub=stub)
    assert result.returncode == 2, f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}"
    results = _read_results(tmp_path)
    assert results["exit_reason"] == "encoder_invalid_halt"
    known_unsat = [r for r in results["runs"] if r["shape_id"] == "fixture-known-unsat-soundness-sanity"]
    assert known_unsat and known_unsat[0]["smt_result"] == "ENCODER_INVALID"


# ---------------------------------------------------------------------------
# Test (6) — smt_timeout_inconclusive (>=1 TIMEOUT_EXHAUSTED row)
# ---------------------------------------------------------------------------

def test_smt_timeout_inconclusive(tmp_path: Path) -> None:
    _scaffold_tmpdir(tmp_path, shapes={"shape-a": "(check-sat)\n"})
    stub = _make_stub_solver(tmp_path, "stub-solver.sh", _stub_timeout_on_shape())
    result = _invoke_runner(
        tmpdir=tmp_path,
        solver_stub=stub,
        extra_args=["--initial-timeout-seconds=1", "--max-retries=0"],
    )
    assert result.returncode == 2, f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}"
    results = _read_results(tmp_path)
    assert results["exit_reason"] == "smt_timeout_inconclusive"
    timeout_rows = [r for r in results["runs"] if r["smt_result"] == "TIMEOUT_EXHAUSTED"]
    assert timeout_rows, "expected >=1 TIMEOUT_EXHAUSTED row"


# ---------------------------------------------------------------------------
# Test (7) — counterexample_infrastructure_failed (solver/scaffold missing)
# ---------------------------------------------------------------------------

def test_counterexample_infrastructure_failed_no_solver(tmp_path: Path) -> None:
    _scaffold_tmpdir(tmp_path, shapes={})
    # Provide a non-existent solver path; runner must refuse.
    stub_path = tmp_path / "nonexistent-solver"
    result = _invoke_runner(tmpdir=tmp_path, solver_stub=stub_path)
    assert result.returncode == 3, f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}"
    # results.json may or may not be populated depending on phase; the
    # test-results envelope IS load-bearing.
    envelope = json.loads((tmp_path / "test-results" / "section-10-result.json").read_text())
    assert envelope["exit_reason"] == "counterexample_infrastructure_failed"


def test_counterexample_infrastructure_failed_fixture_missing(tmp_path: Path) -> None:
    # Scaffold WITHOUT the fixture symlink; runner must halt with
    # counterexample_infrastructure_failed at the encoder-validation
    # preflight (fixture file missing).
    (tmp_path / "scripts").mkdir()
    (tmp_path / "scripts" / "run-section-10-counterexample.sh").symlink_to(RUNNER)
    (tmp_path / "proofs" / "10-counterexample" / "fixtures").mkdir(parents=True)
    # Deliberately omit encoder-validation.smt2 symlink.
    (tmp_path / "test-results").mkdir()
    stub = _make_stub_solver(
        tmp_path,
        "stub-solver.sh",
        '#!/usr/bin/env bash\necho "unsat"\n',
    )
    result = _invoke_runner(tmpdir=tmp_path, solver_stub=stub)
    assert result.returncode == 3, f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}"
    envelope = json.loads((tmp_path / "test-results" / "section-10-result.json").read_text())
    assert envelope["exit_reason"] == "counterexample_infrastructure_failed"
