"""Dual-discharge verdict comparison for the .proof <-> Lean gate.

Orchestrates the dual-discharge contract:
  1. Run the statement-parity prelude (statement_parity.check_parity) over the
     full corpus. ANY parity failure aborts -> dual_discharge_divergence + the
     parity class + theorem ID. The verdict-only comparison is UNSOUND without
     this prelude: a Lean module proving `True` passes its own lake build and
     would falsely "agree" with an Ori-valid verdict.
  2. ONLY on full-corpus parity PASS: compute the Ori-checker verdict (per
     mapped theorem) AND the Lean verdict (lake build of the mapped modules);
     compare per theorem. AGREE -> dual_discharge_agree (exit 0). DISAGREE
     (Ori valid + Lean reject, or vice versa) -> dual_discharge_divergence
     (exit 1) citing the divergent theorem + both verdicts. HARD failure, never
     a soft warning.

Verdict sources (default = computed; override hooks make divergence injectable
for the negative-pin tests):
  Ori:  default = aims-proof/scripts/check-proofs.sh exit 0 -> all "valid";
        override via --ori-verdicts <json> ({theorem_key: "valid"|"reject"}).
  Lean: default = lake build of the lean root exit 0 -> all mapped "valid";
        override via --lean-verdicts <json> ({theorem_key: "valid"|"reject"}).

Theorem key = "<lean_module>::<lean_theorem>" for parity-enforced rows.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys

from statement_parity import check_parity, _PARITY_ENFORCED_KINDS


def _load_verdict_override(path: str | None) -> dict[str, str] | None:
    if path is None:
        return None
    with open(path, encoding="utf-8") as fh:
        return json.load(fh)


def _theorem_keys(map_path: str) -> list[str]:
    data = json.load(open(map_path, encoding="utf-8"))
    keys = []
    for row in data["rows"]:
        if row["kind"] in _PARITY_ENFORCED_KINDS and row.get("lean_theorem"):
            keys.append(f"{row['lean_module']}::{row['lean_theorem']}")
    return keys


def _run_ori_checker(aims_proof_dir: str) -> bool:
    """True when the Ori proof checker reports the corpus clean (exit 0)."""
    proc = subprocess.run(
        ["bash", os.path.join(aims_proof_dir, "scripts", "check-proofs.sh")],
        cwd=aims_proof_dir,
        capture_output=True,
        text=True,
    )
    return proc.returncode == 0


def _run_lake_build(lean_root: str) -> bool:
    """True when `lake build` of the Lean project succeeds (exit 0)."""
    lake = _which("lake")
    if lake is None:
        raise RuntimeError("lake not on PATH; cannot compute Lean verdict")
    proc = subprocess.run(
        [lake, "build"],
        cwd=lean_root,
        capture_output=True,
        text=True,
    )
    return proc.returncode == 0


def _which(prog: str) -> str | None:
    from shutil import which
    return which(prog)


def dual_discharge(
    map_path: str,
    proofs_dir: str,
    lean_root: str,
    ori_verdicts: dict[str, str] | None,
    lean_verdicts: dict[str, str] | None,
    aims_proof_dir: str,
) -> dict:
    # ---- Stage 1: statement-parity prelude (load-bearing; runs FIRST). ----
    parity = check_parity(map_path, proofs_dir, lean_root)
    if not parity.passed:
        first = parity.failures[0]
        return {
            "exit_reason": "dual_discharge_divergence",
            "stage": "statement_parity",
            "passed": False,
            "parity_class": first["parity_class"],
            "theorem": first.get("theorem"),
            "proof_path": first.get("proof_path"),
            "failures": parity.failures,
        }

    # ---- Stage 2: verdict comparison (only on full-corpus parity PASS). ----
    keys = _theorem_keys(map_path)

    if ori_verdicts is None:
        ori_clean = _run_ori_checker(aims_proof_dir)
        ori_verdicts = {k: ("valid" if ori_clean else "reject") for k in keys}
    if lean_verdicts is None:
        lean_clean = _run_lake_build(lean_root)
        lean_verdicts = {k: ("valid" if lean_clean else "reject") for k in keys}

    divergences = []
    for k in keys:
        ori_v = ori_verdicts.get(k, "valid")
        lean_v = lean_verdicts.get(k, "valid")
        if ori_v != lean_v:
            divergences.append({"theorem": k, "ori_verdict": ori_v, "lean_verdict": lean_v})

    if divergences:
        return {
            "exit_reason": "dual_discharge_divergence",
            "stage": "verdict_comparison",
            "passed": False,
            "divergences": divergences,
        }

    return {
        "exit_reason": "dual_discharge_agree",
        "stage": "verdict_comparison",
        "passed": True,
        "theorems_compared": len(keys),
    }


def main(argv: list[str] | None = None) -> int:
    script_dir = os.path.dirname(os.path.abspath(__file__))
    aims_proof_dir = os.path.dirname(script_dir)
    parser = argparse.ArgumentParser(description="Dual-discharge verdict comparison.")
    parser.add_argument("--map", default=os.path.join(script_dir, "proof-lean-map.json"))
    parser.add_argument("--proofs-dir", default=os.path.join(aims_proof_dir, "proofs"))
    parser.add_argument("--lean-root", default=os.path.join(aims_proof_dir, "lean"))
    parser.add_argument("--aims-proof-dir", default=aims_proof_dir)
    parser.add_argument("--ori-verdicts", default=None,
                        help="JSON file overriding per-theorem Ori verdicts")
    parser.add_argument("--lean-verdicts", default=None,
                        help="JSON file overriding per-theorem Lean verdicts")
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args(argv)

    result = dual_discharge(
        args.map,
        args.proofs_dir,
        args.lean_root,
        _load_verdict_override(args.ori_verdicts),
        _load_verdict_override(args.lean_verdicts),
        args.aims_proof_dir,
    )

    if args.json:
        print(json.dumps(result, indent=2))
    elif result["passed"]:
        print(
            f"dual-discharge: dual_discharge_agree — "
            f"{result['theorems_compared']} theorems agree across Ori-checker + Lean."
        )
    else:
        if result["stage"] == "statement_parity":
            print(
                f"dual-discharge: dual_discharge_divergence — statement-parity "
                f"failure [{result['parity_class']}] on "
                f"{result.get('theorem') or result.get('proof_path')}",
                file=sys.stderr,
            )
        else:
            print(
                "dual-discharge: dual_discharge_divergence — verdict disagreement:",
                file=sys.stderr,
            )
            for d in result["divergences"]:
                print(
                    f"  - {d['theorem']}: Ori={d['ori_verdict']} Lean={d['lean_verdict']}",
                    file=sys.stderr,
                )

    return 0 if result["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
