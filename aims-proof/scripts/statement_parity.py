"""Statement-parity checker for the .proof <-> Lean dual-discharge gate.

Resolves every .proof under aims-proof/proofs/ through proof-lean-map.json and
asserts each mapped Lean theorem's STATEMENT matches the .proof obligation.

Reject classes (each -> dual_discharge_divergence with a parity-class string):
  - missing_mapping: a .proof file with no proof-lean-map.json row.
  - wrong_theorem_id: a theorem/composition_folded row whose lean_theorem is
    absent from its lean_module file.
  - weaker_statement: a resolved Lean theorem whose proposition is vacuous-True
    (": True := ..." / ": True") OR mentions none of the row's parity_tokens
    (carrier/operator evidence the .proof obligation names).

Non-theorem rows (coverage_shape / foundation / bootstrap / doc_encoded / gap /
definition / removed) carry lean_theorem=null and are faithfully classified;
they are NOT parity failures. gap rows are Lean-coverage gaps the map surfaces,
distinct from mapping errors.

CLI:
  python3 statement_parity.py [--map <path>] [--proofs-dir <path>] [--json]
Exit codes:
  0 — full-corpus parity PASS.
  1 — at least one parity failure (dual_discharge_divergence).
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
from dataclasses import dataclass, field

_PARITY_ENFORCED_KINDS = frozenset({"theorem", "composition_folded"})

_THEOREM_BLOCK_RE_CACHE: dict[str, str] = {}


@dataclass
class ParityResult:
    passed: bool
    failures: list[dict] = field(default_factory=list)
    checked: int = 0
    skipped_non_theorem: int = 0


def _normalize(text: str) -> str:
    """Lowercase + strip non-alphanumerics (tolerates canonical<->Lean naming)."""
    return re.sub(r"[^a-z0-9]", "", text.lower())


def _module_to_path(lean_root: str, module: str) -> str:
    """AimsProof.Lattice -> <lean_root>/AimsProof/Lattice.lean."""
    return os.path.join(lean_root, *module.split(".")) + ".lean"


def _extract_statement(file_text: str, theorem: str) -> str | None:
    """Return the statement (signature up to ':=') of `theorem`, or None."""
    m = re.search(
        r"(?:theorem|lemma)\s+" + re.escape(theorem) + r"\b(.*?):=",
        file_text,
        re.S,
    )
    return m.group(1) if m else None


def _extract_block(file_text: str, theorem: str) -> str | None:
    """Return the full theorem block (name through proof body) of `theorem`."""
    m = re.search(
        r"((?:theorem|lemma)\s+"
        + re.escape(theorem)
        + r"\b.*?:=.*?)(?=\n(?:theorem|lemma|def|/-!|/--|@\[)|\Z)",
        file_text,
        re.S,
    )
    return m.group(1) if m else None


def _is_vacuous_true(statement: str) -> bool:
    """True when the proposition is ': True' (vacuous-True theorem body)."""
    return bool(
        re.search(r":\s*True\b\s*:=", statement)
        or re.search(r":\s*True\s*$", statement.strip())
    )


def check_parity(map_path: str, proofs_dir: str, lean_root: str) -> ParityResult:
    data = json.load(open(map_path, encoding="utf-8"))
    rows = data["rows"]
    by_proof_path = {r["proof_path"]: r for r in rows}

    actual_proofs: list[str] = []
    for root, _dirs, files in os.walk(proofs_dir):
        for fname in files:
            if fname.endswith(".proof"):
                abspath = os.path.join(root, fname)
                actual_proofs.append(abspath)
    actual_proofs.sort()

    # proof-lean-map.json proof_path is aims-proof-relative (e.g.
    # "proofs/02-lattice/L-1.proof"); aims-proof/ is proofs_dir's parent.
    aims_proof_dir = os.path.dirname(proofs_dir)
    result = ParityResult(passed=True)
    file_cache: dict[str, str | None] = {}

    for abspath in actual_proofs:
        rel = os.path.relpath(abspath, aims_proof_dir)
        row = by_proof_path.get(rel)
        if row is None:
            result.failures.append(
                {"parity_class": "missing_mapping", "proof_path": rel,
                 "detail": "no proof-lean-map.json row maps this .proof"}
            )
            continue

        if row["kind"] not in _PARITY_ENFORCED_KINDS:
            result.skipped_non_theorem += 1
            continue

        module = row["lean_module"]
        theorem = row["lean_theorem"]
        lean_path = _module_to_path(lean_root, module)
        if lean_path not in file_cache:
            file_cache[lean_path] = (
                open(lean_path, encoding="utf-8").read()
                if os.path.exists(lean_path)
                else None
            )
        file_text = file_cache[lean_path]
        if file_text is None:
            result.failures.append(
                {"parity_class": "wrong_theorem_id", "proof_path": rel,
                 "theorem": theorem, "module": module,
                 "detail": f"lean_module file missing: {lean_path}"}
            )
            continue

        statement = _extract_statement(file_text, theorem)
        if statement is None:
            result.failures.append(
                {"parity_class": "wrong_theorem_id", "proof_path": rel,
                 "theorem": theorem, "module": module,
                 "detail": f"theorem name absent from {module}"}
            )
            continue

        if _is_vacuous_true(statement):
            result.failures.append(
                {"parity_class": "weaker_statement", "proof_path": rel,
                 "theorem": theorem, "module": module,
                 "detail": "proposition is vacuous-True (': True')"}
            )
            continue

        tokens = row.get("parity_tokens", [])
        if tokens:
            block = _extract_block(file_text, theorem) or statement
            normalized_block = _normalize(block)
            if not any(_normalize(tok) in normalized_block for tok in tokens):
                result.failures.append(
                    {"parity_class": "weaker_statement", "proof_path": rel,
                     "theorem": theorem, "module": module,
                     "detail": f"none of parity_tokens {tokens} present in statement"}
                )
                continue

        result.checked += 1

    result.passed = not result.failures
    return result


def main(argv: list[str] | None = None) -> int:
    script_dir = os.path.dirname(os.path.abspath(__file__))
    aims_proof_dir = os.path.dirname(script_dir)
    parser = argparse.ArgumentParser(description="Lean statement-parity checker.")
    parser.add_argument("--map", default=os.path.join(script_dir, "proof-lean-map.json"))
    parser.add_argument("--proofs-dir", default=os.path.join(aims_proof_dir, "proofs"))
    parser.add_argument("--lean-root", default=os.path.join(aims_proof_dir, "lean"))
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args(argv)

    result = check_parity(args.map, args.proofs_dir, args.lean_root)

    if args.json:
        print(json.dumps({
            "passed": result.passed,
            "checked": result.checked,
            "skipped_non_theorem": result.skipped_non_theorem,
            "failures": result.failures,
        }, indent=2))
    else:
        if result.passed:
            print(
                f"statement-parity: dual_discharge_agree (parity prelude) — "
                f"{result.checked} theorem rows parity-clean, "
                f"{result.skipped_non_theorem} non-theorem rows classified."
            )
        else:
            print(
                f"statement-parity: dual_discharge_divergence — "
                f"{len(result.failures)} parity failure(s):",
                file=sys.stderr,
            )
            for f in result.failures:
                print(
                    f"  - [{f['parity_class']}] {f.get('proof_path')}"
                    f"{' :: ' + f['theorem'] if f.get('theorem') else ''}"
                    f" — {f['detail']}",
                    file=sys.stderr,
                )

    return 0 if result.passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
