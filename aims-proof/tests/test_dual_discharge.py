"""Durable regression test for the dual-discharge gate.

Verifies dual-discharge.sh (via dual_discharge.py + statement_parity.py):
  POSITIVE pin — the real corpus, full statement-parity PASS + injected agreeing
    verdicts -> exit 0 (dual_discharge_agree).
  NEGATIVE pins (one per parity class + one verdict divergence):
    (1) a scratch .proof with NO map row -> exit 1, missing_mapping.
    (2) a map row pointing at a non-existent Lean theorem -> exit 1,
        wrong_theorem_id.
    (3) a scratch Lean module proving `: True := by trivial` for a mapped
        theorem -> exit 1, weaker_statement (pins that the verdict-only gate
        would have falsely passed it — the parity prelude is load-bearing).
    (4) an injected verdict divergence (Lean reject + Ori valid) on a
        parity-passing theorem -> exit 1, citing the divergent theorem.

Scratch-dir discipline mirrors test_check_proofs_gate.py: every scratch corpus
is staged under a unique tmp dir and removed on teardown; the real
aims-proof/proofs/ + aims-proof/lean/ are NEVER mutated.
"""

from __future__ import annotations

import json
import shutil
import subprocess
from pathlib import Path

import pytest

SCRIPTS_REL = "aims-proof/scripts"


def _aims_proof_dir(workspace_root: Path) -> Path:
    return workspace_root / "aims-proof"


def _run_dual_discharge(workspace_root: Path, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["bash", f"{SCRIPTS_REL}/dual-discharge.sh", *args],
        cwd=str(workspace_root),
        capture_output=True,
        text=True,
        timeout=140,
    )


def _theorem_keys(workspace_root: Path) -> list[str]:
    map_path = _aims_proof_dir(workspace_root) / "scripts" / "proof-lean-map.json"
    data = json.loads(map_path.read_text())
    return [
        f"{r['lean_module']}::{r['lean_theorem']}"
        for r in data["rows"]
        if r["kind"] in ("theorem", "composition_folded") and r.get("lean_theorem")
    ]


# ---------------------------------------------------------------------------
# Scratch-corpus fixture: a minimal map + proofs-dir + lean-root that the
# parity helper reads via --map / --proofs-dir / --lean-root. Real corpus
# untouched.
# ---------------------------------------------------------------------------

@pytest.fixture
def scratch_corpus(workspace_root: Path, tmp_path: Path):
    """Yield a builder that stages a scratch (map, proofs_dir, lean_root) and
    returns the CLI args pointing dual-discharge.sh at it."""
    scratch = tmp_path / "dd_scratch"
    proofs_dir = scratch / "proofs" / "02-lattice"
    lean_dir = scratch / "lean" / "AimsProof"
    proofs_dir.mkdir(parents=True)
    lean_dir.mkdir(parents=True)

    def build(rows: list[dict], proof_files: dict[str, str], lean_modules: dict[str, str]):
        map_path = scratch / "proof-lean-map.json"
        map_path.write_text(json.dumps({
            "schema": "proof-lean-map/v1",
            "description": "scratch",
            "rows": rows,
        }))
        for rel, body in proof_files.items():
            p = scratch / rel
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(body)
        for module_file, body in lean_modules.items():
            p = scratch / "lean" / "AimsProof" / module_file
            p.write_text(body)
        return [
            "--map", str(map_path),
            "--proofs-dir", str(scratch / "proofs"),
            "--lean-root", str(scratch / "lean"),
        ]

    try:
        yield build
    finally:
        shutil.rmtree(scratch, ignore_errors=True)


_GOOD_LEAN = (
    "theorem L1_join_comm (a b : AimsState) : a.join b = b.join a := by\n"
    "  rfl\n"
)
_TRUE_LEAN = (
    "theorem L1_join_comm : True := by trivial\n"
)
_PROOF_BODY = "Theorem L-1: Lattice join commutativity\n  status: valid\n"


# ---------------------------------------------------------------------------
# POSITIVE pin — real corpus, injected agreeing verdicts.
# ---------------------------------------------------------------------------

def test_positive_real_corpus_agrees(workspace_root: Path, tmp_path: Path) -> None:
    keys = _theorem_keys(workspace_root)
    all_valid = {k: "valid" for k in keys}
    ori = tmp_path / "ori.json"
    lean = tmp_path / "lean.json"
    ori.write_text(json.dumps(all_valid))
    lean.write_text(json.dumps(all_valid))

    result = _run_dual_discharge(
        workspace_root, "--ori-verdicts", str(ori), "--lean-verdicts", str(lean)
    )
    combined = result.stdout + result.stderr
    assert result.returncode == 0, f"expected exit 0; got {result.returncode}; {combined!r}"
    assert "dual_discharge_agree" in combined, combined


# ---------------------------------------------------------------------------
# NEGATIVE pin (1) — missing mapping.
# ---------------------------------------------------------------------------

def test_negative_missing_mapping(workspace_root: Path, scratch_corpus) -> None:
    # A .proof with NO row in the map.
    args = scratch_corpus(
        rows=[],
        proof_files={"proofs/02-lattice/L-1.proof": _PROOF_BODY},
        lean_modules={"Lattice.lean": _GOOD_LEAN},
    )
    result = _run_dual_discharge(workspace_root, *args)
    combined = result.stdout + result.stderr
    assert result.returncode == 1, f"expected exit 1; got {result.returncode}; {combined!r}"
    assert "dual_discharge_divergence" in combined, combined
    assert "missing_mapping" in combined, combined


# ---------------------------------------------------------------------------
# NEGATIVE pin (2) — wrong theorem id.
# ---------------------------------------------------------------------------

def test_negative_wrong_theorem_id(workspace_root: Path, scratch_corpus) -> None:
    args = scratch_corpus(
        rows=[{
            "proof_id": "L-1", "proof_path": "proofs/02-lattice/L-1.proof",
            "kind": "theorem", "lean_module": "AimsProof.Lattice",
            "lean_theorem": "L1_does_not_exist", "parity_tokens": ["AimsState", "join"],
            "note": "",
        }],
        proof_files={"proofs/02-lattice/L-1.proof": _PROOF_BODY},
        lean_modules={"Lattice.lean": _GOOD_LEAN},
    )
    result = _run_dual_discharge(workspace_root, *args)
    combined = result.stdout + result.stderr
    assert result.returncode == 1, f"expected exit 1; got {result.returncode}; {combined!r}"
    assert "dual_discharge_divergence" in combined, combined
    assert "wrong_theorem_id" in combined, combined


# ---------------------------------------------------------------------------
# NEGATIVE pin (3) — weaker (vacuous-True) statement. Pins that a verdict-only
# gate would falsely pass: the True-shaped module builds clean yet the parity
# prelude rejects it BEFORE any verdict comparison.
# ---------------------------------------------------------------------------

def test_negative_weaker_true_statement(workspace_root: Path, scratch_corpus) -> None:
    args = scratch_corpus(
        rows=[{
            "proof_id": "L-1", "proof_path": "proofs/02-lattice/L-1.proof",
            "kind": "theorem", "lean_module": "AimsProof.Lattice",
            "lean_theorem": "L1_join_comm", "parity_tokens": ["AimsState", "join"],
            "note": "",
        }],
        proof_files={"proofs/02-lattice/L-1.proof": _PROOF_BODY},
        lean_modules={"Lattice.lean": _TRUE_LEAN},
    )
    result = _run_dual_discharge(workspace_root, *args)
    combined = result.stdout + result.stderr
    assert result.returncode == 1, f"expected exit 1; got {result.returncode}; {combined!r}"
    assert "dual_discharge_divergence" in combined, combined
    assert "weaker_statement" in combined, combined


# ---------------------------------------------------------------------------
# NEGATIVE pin (4) — verdict divergence (Lean reject + Ori valid) on a
# parity-passing theorem. Real corpus passes parity; injected verdicts diverge.
# ---------------------------------------------------------------------------

def test_negative_verdict_divergence(workspace_root: Path, tmp_path: Path) -> None:
    keys = _theorem_keys(workspace_root)
    target = keys[0]
    ori = tmp_path / "ori.json"
    lean = tmp_path / "lean.json"
    ori.write_text(json.dumps({k: "valid" for k in keys}))
    lean.write_text(json.dumps({**{k: "valid" for k in keys}, target: "reject"}))

    result = _run_dual_discharge(
        workspace_root, "--ori-verdicts", str(ori), "--lean-verdicts", str(lean)
    )
    combined = result.stdout + result.stderr
    assert result.returncode == 1, f"expected exit 1; got {result.returncode}; {combined!r}"
    assert "dual_discharge_divergence" in combined, combined
    assert target in combined, f"divergent theorem {target!r} not cited: {combined!r}"
