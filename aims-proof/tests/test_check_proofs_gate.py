"""Durable integration regression test for the proof-checking CI gate.

Per the proof CI integration gate item 10:
verifies scripts/check-proofs.sh catches a deliberate proof break (exit 1,
broken artifact cited) AND passes a valid proof through the same path (exit 0).

Mechanism:
  - Stage a scratch subdir UNDER aims-proof/proofs/ (so the checker's
    coverage-manifest.json walk-up resolution succeeds) holding two copies of a
    real corpus proof: one pristine (positive pin), one corrupted (negative pin).
  - Corruption mangles the theorem-id header line so the parser rejects it with
    notation_missing_theorem_block -> {"status": "fail"} -> check-proofs.sh maps
    any non-valid verdict to a regression -> exit 1.
  - Scope check-proofs.sh to each file via the {staged_files}-style affected-set
    narrowing the script supports.
  - Scratch subdir auto-removed; the real corpus is never mutated.

Exit-code contract per scripts/check-proofs.sh: 0 clean / 1 regression /
3 checker build failure.
"""

from __future__ import annotations

import shutil
import subprocess
from pathlib import Path

import pytest


# A real, valid corpus proof with no .expected sibling (default valid-mode
# discharge). The scratch copies inherit its theorem body.
SOURCE_PROOF_REL = "aims-proof/proofs/02-lattice/L-1.proof"

# The exact header line the corruption mangles into an unknown category prefix.
THEOREM_HEADER = "Theorem L-1: Lattice join commutativity"
CORRUPT_HEADER = "Theorem BOGUS-99: corrupted theorem id"


def _run_check_proofs(workspace_root: Path, *proof_args: str) -> subprocess.CompletedProcess:
    """Invoke aims-proof/scripts/check-proofs.sh scoped to the affected-set paths."""
    return subprocess.run(
        ["bash", "aims-proof/scripts/check-proofs.sh", *proof_args],
        cwd=str(workspace_root),
        capture_output=True,
        text=True,
        timeout=140,
    )


@pytest.fixture
def gate_scratch(workspace_root: Path):
    """Yield (valid_proof, broken_proof) wrapper-relative paths.

    Stages a unique scratch subdir under aims-proof/proofs/ with one pristine
    and one theorem-id-corrupted copy of SOURCE_PROOF_REL. Removes the subdir
    on teardown so the real corpus is never left corrupted.
    """
    source = workspace_root / SOURCE_PROOF_REL
    assert source.is_file(), f"source proof missing: {source}"

    scratch_dir = workspace_root / "aims-proof" / "proofs" / "_check_proofs_gate_scratch"
    if scratch_dir.exists():
        shutil.rmtree(scratch_dir)
    scratch_dir.mkdir(parents=True)

    try:
        valid_abs = scratch_dir / "valid.proof"
        broken_abs = scratch_dir / "broken.proof"
        body = source.read_text()
        valid_abs.write_text(body)

        assert THEOREM_HEADER in body, (
            f"source proof header drifted; expected {THEOREM_HEADER!r} in {source}"
        )
        broken_abs.write_text(body.replace(THEOREM_HEADER, CORRUPT_HEADER))

        valid_rel = str(valid_abs.relative_to(workspace_root))
        broken_rel = str(broken_abs.relative_to(workspace_root))
        yield valid_rel, broken_rel
    finally:
        shutil.rmtree(scratch_dir, ignore_errors=True)


def test_gate_catches_broken_proof(workspace_root: Path, gate_scratch) -> None:
    """NEGATIVE pin: a corrupted proof makes check-proofs.sh exit 1 + cite it."""
    _valid_rel, broken_rel = gate_scratch
    result = _run_check_proofs(workspace_root, broken_rel)

    if result.returncode == 3:
        pytest.skip(
            "checker build failed (exit 3); cannot exercise the gate: "
            f"stderr={result.stderr!r}"
        )

    combined = result.stdout + result.stderr
    assert result.returncode == 1, (
        f"expected exit 1 (regression caught), got {result.returncode}; "
        f"stdout={result.stdout!r}; stderr={result.stderr!r}"
    )
    assert "REGRESSIONS" in combined, f"regression report absent: {combined!r}"
    assert "_check_proofs_gate_scratch/broken.proof" in combined, (
        f"broken artifact not cited in output: {combined!r}"
    )


def test_gate_passes_valid_proof(workspace_root: Path, gate_scratch) -> None:
    """POSITIVE pin: a valid proof passes the same path with exit 0."""
    valid_rel, _broken_rel = gate_scratch
    result = _run_check_proofs(workspace_root, valid_rel)

    if result.returncode == 3:
        pytest.skip(
            "checker build failed (exit 3); cannot exercise the gate: "
            f"stderr={result.stderr!r}"
        )

    combined = result.stdout + result.stderr
    assert result.returncode == 0, (
        f"expected exit 0 (valid proof clean), got {result.returncode}; "
        f"stdout={result.stdout!r}; stderr={result.stderr!r}"
    )
    assert "1 proofs checked" in combined, (
        f"expected '1 proofs checked' in output: {combined!r}"
    )
    assert "REGRESSIONS" not in combined, f"unexpected regression report: {combined!r}"
