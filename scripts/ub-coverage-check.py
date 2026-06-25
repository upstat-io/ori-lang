#!/usr/bin/env python3
"""ub-coverage-check — undefined-behavior disposition coverage checker.

Reads the UB coverage matrix (scripts/ub-safety/coverage-matrix.json), validates
it against the matrix schema, and checks every row's disposition is grounded and
pinned. The matrix records, per miri-enumerated UB class, where the proof that
the class cannot occur stops (its disposition) and the Ori-side pin that proves
the foreclosure or anchors the obligation/gap. Spec Clause 4.5 (no silent
undefined behavior).

A row is green only when:
  - the matrix is schema-valid (every row has a class id, a miri source cite, a
    disposition in the taxonomy, an Ori mechanism, and a pin ref); and
  - the row carries a non-empty pin ref (a gap row with no hardening anchor fails
    closed); and
  - the pin ref resolves to a real file (or file::symbol) on disk.

`--strict` additionally requires COMPLETENESS: every canonical UB class carries
at least one row. The seed matrix is intentionally incomplete, so `--strict`
reads red until the disposition rows are fully populated; `--seed` reads green
over the seed set.

Disposition taxonomy (the matrix schema is the source of truth):
  foreclosed-typesystem  the type system makes the UB unrepresentable
  aims-obligation        foreclosed at the type level; obligation discharged at
                         codegen/runtime by AIMS, pinned by a test
  data-race-foreclosed   value semantics + Sendable forbid the data race
  ffi-unsafe-boundary    foreclosure stops at the FFI boundary; checked there
  gap                    neither foreclosed nor obligated yet; MUST cite a
                         hardening anchor (fails closed without one)

Modes (default: report every row):
  --seed          Check the seed set; emit PASS/FAIL as the final stdout line.
  --bucket AREA   Check one disposition area (foreclosed-typesystem |
                  aims-obligation | concurrency | ffi-unsafe-boundary); PASS/FAIL.
  --strict        Check every row AND completeness over the canonical UB classes.
  --summary       With --strict: print a one-line coverage summary to stderr.
  --json          Emit the full result as JSON to stdout.
  --self-test     Run the built-in self-test and exit.

Flags:
  --matrix FILE   Matrix data file (default: scripts/ub-safety/coverage-matrix.json)
  --schema FILE   Matrix schema (default: scripts/ub-safety/coverage-matrix.schema.json)
  --warn-only     Always exit 0; print findings to stderr (bedding-in mode).

The probe modes (--seed / --bucket / --strict) print PASS or FAIL as the final
non-empty stdout line so a success-criterion probe reads it directly; human
detail goes to stderr.

Exit codes:
  0  clean (green) or --warn-only
  1  violations found (schema error, ungrounded/unpinned/unresolving row, or
     --strict completeness gap)
  2  error (bad args, missing file, jsonschema unavailable)
"""

import argparse
import json
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_MATRIX = REPO_ROOT / "scripts" / "ub-safety" / "coverage-matrix.json"
DEFAULT_SCHEMA = REPO_ROOT / "scripts" / "ub-safety" / "coverage-matrix.schema.json"

# The canonical UB class set: rustc UndefinedBehaviorInfo (37 variants,
# rustc_middle/src/mir/interpret/error.rs) plus the miri runtime detectors for
# the concurrency classes the type system permits. --strict completeness is
# measured against this set; it is the denominator of the coverage metric.
CANONICAL_UB_CLASSES = frozenset({
    "Ub", "Custom", "ValidationError", "Unreachable", "BoundsCheckFailed",
    "DivisionByZero", "RemainderByZero", "DivisionOverflow", "RemainderOverflow",
    "PointerArithOverflow", "ArithOverflow", "ShiftOverflow", "InvalidMeta",
    "UnterminatedCString", "PointerUseAfterFree", "PointerOutOfBounds",
    "DanglingIntPointer", "AlignmentCheckFailed", "WriteToReadOnly",
    "DerefFunctionPointer", "DerefVTablePointer", "DerefTypeIdPointer",
    "InvalidBool", "InvalidChar", "InvalidTag", "InvalidFunctionPointer",
    "InvalidVTablePointer", "InvalidVTableTrait", "InvalidStr",
    "InvalidUninitBytes", "DeadLocal", "ScalarSizeMismatch",
    "UninhabitedEnumVariantWritten", "UninhabitedEnumVariantRead",
    "InvalidNichedEnumVariantWritten", "AbiMismatchArgument", "AbiMismatchReturn",
    # miri runtime detectors (no rustc UB-enum variant; the type system permits
    # them, so they are detected at runtime — the concurrency disposition class).
    "DataRace", "WeakMemoryOrdering",
})

# A disposition "area" (the section/bucket name a success-criterion probe uses)
# maps to the set of dispositions whose rows it covers.
BUCKET_AREAS = {
    "foreclosed-typesystem": {"foreclosed-typesystem"},
    "aims-obligation": {"aims-obligation"},
    "concurrency": {"data-race-foreclosed", "gap"},
    "ffi-unsafe-boundary": {"ffi-unsafe-boundary"},
}


def load_json(path):
    """Return the parsed JSON at path; raises FileNotFoundError / ValueError."""
    with open(path, "r", encoding="utf-8") as fh:
        return json.load(fh)


def schema_errors(matrix, schema):
    """Return a sorted list of schema-validation error strings (empty = valid)."""
    from jsonschema import Draft202012Validator

    validator = Draft202012Validator(schema)
    errors = []
    for err in validator.iter_errors(matrix):
        loc = "/".join(str(p) for p in err.absolute_path) or "<root>"
        errors.append(f"{loc}: {err.message}")
    return sorted(errors)


def resolve_pin(pin_ref, root):
    """Resolve a pin ref against root.

    Format: a repo-relative file path, optionally suffixed with `::symbol`.
    Returns (resolved: bool, detail: str).
    """
    ref = pin_ref.strip()
    if not ref:
        return False, "empty pin ref"
    file_part, _, symbol = ref.partition("::")
    target = root / file_part
    if not target.is_file():
        return False, f"file not found: {file_part}"
    if symbol:
        try:
            text = target.read_text(encoding="utf-8", errors="replace")
        except OSError as exc:
            return False, f"unreadable: {file_part} ({exc})"
        if symbol not in text:
            return False, f"symbol not found: {symbol} in {file_part}"
    return True, "resolved"


def check_row(row, root):
    """Return (ok: bool, reason: str) for one matrix row (schema already valid)."""
    disposition = row.get("disposition", "")
    pin_ref = (row.get("pin_ref") or "").strip()
    if not pin_ref:
        if disposition == "gap":
            return False, "gap row missing hardening anchor (fails closed)"
        return False, f"{disposition} row not pinned"
    resolved, detail = resolve_pin(pin_ref, root)
    if not resolved:
        return False, f"pin does not resolve: {detail}"
    return True, "grounded and pinned"


def run_check(matrix_path, schema_path, root=REPO_ROOT, *, dispositions=None,
              strict=False):
    """Run the check; return a result dict (no IO side effects).

    dispositions: when given, restrict the checked rows to these disposition
                  values (a `--bucket` area). None checks every row.
    strict:       additionally require completeness over CANONICAL_UB_CLASSES.
    """
    matrix = load_json(matrix_path)
    schema = load_json(schema_path)

    serrs = schema_errors(matrix, schema)
    rows_report = []
    bucket_empty = False
    missing_classes = []
    if not serrs:
        all_rows = matrix.get("rows", [])
        rows = all_rows
        if dispositions is not None:
            rows = [r for r in all_rows if r.get("disposition") in dispositions]
            bucket_empty = len(rows) == 0
        for row in rows:
            ok, reason = check_row(row, root)
            rows_report.append({
                "class_id": row.get("class_id", "<unknown>"),
                "disposition": row.get("disposition", "<unknown>"),
                "ok": ok,
                "reason": reason,
            })
        if strict:
            present = {r.get("class_id") for r in all_rows}
            missing_classes = sorted(CANONICAL_UB_CLASSES - present)

    violations = (
        bool(serrs)
        or any(not r["ok"] for r in rows_report)
        or bucket_empty
        or bool(missing_classes)
    )
    return {
        "schema_errors": serrs,
        "rows": rows_report,
        "row_count": len(rows_report),
        "bucket_empty": bucket_empty,
        "strict": strict,
        "missing_classes": missing_classes,
        "canonical_total": len(CANONICAL_UB_CLASSES),
        "green": not violations,
    }


def format_text(result):
    """Render the result as a human-readable row-per-class report."""
    lines = []
    if result["schema_errors"]:
        lines.append("SCHEMA INVALID:")
        for err in result["schema_errors"]:
            lines.append(f"  - {err}")
        lines.append("")
    for r in result["rows"]:
        mark = "OK  " if r["ok"] else "FAIL"
        lines.append(f"  {mark}  {r['class_id']} [{r['disposition']}] -> {r['reason']}")
    if result["bucket_empty"]:
        lines.append("  FAIL  (no rows for the requested disposition area)")
    if result["strict"] and result["missing_classes"]:
        lines.append(
            f"  FAIL  completeness: {len(result['missing_classes'])} of "
            f"{result['canonical_total']} UB classes undispositioned"
        )
    verdict = "GREEN" if result["green"] else "RED"
    lines.append("")
    lines.append(f"ub-coverage: {verdict} ({result['row_count']} row(s) checked)")
    return "\n".join(lines) + "\n"


def _coverage_summary(result):
    """One-line completeness summary (for --strict --summary)."""
    present = result["canonical_total"] - len(result["missing_classes"])
    return (f"coverage: {present}/{result['canonical_total']} UB classes "
            f"dispositioned ({'complete' if result['green'] else 'incomplete'})")


# --------------------------------------------------------------------------- #
# Self-test
# --------------------------------------------------------------------------- #

def _self_test():
    """Built-in self-test. Returns 0 on pass, 1 on failure."""
    import tempfile

    schema = load_json(DEFAULT_SCHEMA)
    self_pin = "scripts/ub-coverage-check.py"  # a pin that always resolves
    results = []

    def record(name, ok):
        results.append((name, ok))

    with tempfile.TemporaryDirectory() as td:
        schema_path = Path(td) / "schema.json"
        schema_path.write_text(json.dumps(schema), encoding="utf-8")

        def write_matrix(rows):
            p = Path(td) / "matrix.json"
            p.write_text(json.dumps({"schema_version": 1, "rows": rows}), encoding="utf-8")
            return p

        wf = write_matrix([{
            "class_id": "InvalidBool",
            "miri_source": "rust: error.rs UndefinedBehaviorInfo::InvalidBool",
            "disposition": "foreclosed-typesystem",
            "ori_mechanism": "bool has no invalid bit pattern; no transmute. Spec Clause 4.5.",
            "pin_ref": self_pin,
        }])
        record("well-formed matrix is green", run_check(wf, schema_path)["green"] is True)

        missing = write_matrix([{
            "class_id": "InvalidBool", "miri_source": "rust: error.rs",
            "ori_mechanism": "...", "pin_ref": self_pin,
        }])
        record("missing disposition rejected", bool(run_check(missing, schema_path)["schema_errors"]))

        gap = write_matrix([{
            "class_id": "WeakMemoryOrdering", "miri_source": "miri: tests/fail/weak_memory",
            "disposition": "gap", "ori_mechanism": "no ordering model yet.", "pin_ref": "",
        }])
        gap_res = run_check(gap, schema_path)
        record("gap row without anchor fails closed",
               gap_res["green"] is False and not gap_res["schema_errors"])

        unresolved = write_matrix([{
            "class_id": "InvalidBool", "miri_source": "rust: error.rs",
            "disposition": "foreclosed-typesystem", "ori_mechanism": "...",
            "pin_ref": "compiler/does/not/exist.rs::nope",
        }])
        record("unresolvable pin is red", run_check(unresolved, schema_path)["green"] is False)

        # --bucket: a bucket with rows is checked; an empty bucket fails closed.
        bucket_matrix = write_matrix([{
            "class_id": "AlignmentCheckFailed", "miri_source": "rust: error.rs",
            "disposition": "aims-obligation", "ori_mechanism": "alignment via ReprPlan.",
            "pin_ref": self_pin,
        }])
        record("bucket with rows green",
               run_check(bucket_matrix, schema_path,
                         dispositions=BUCKET_AREAS["aims-obligation"])["green"] is True)
        record("empty bucket fails closed",
               run_check(bucket_matrix, schema_path,
                         dispositions=BUCKET_AREAS["ffi-unsafe-boundary"])["green"] is False)

        # --strict: an incomplete matrix is red on completeness.
        record("strict incomplete matrix is red",
               run_check(bucket_matrix, schema_path, strict=True)["green"] is False)

    # End-to-end over the bundled seed matrix.
    record("bundled seed matrix is green (--seed)",
           run_check(DEFAULT_MATRIX, DEFAULT_SCHEMA)["green"] is True)
    record("bundled matrix is complete (--strict)",
           run_check(DEFAULT_MATRIX, DEFAULT_SCHEMA, strict=True)["green"] is True)

    failed = [n for n, ok in results if not ok]
    for name, ok in results:
        print(f"  {'PASS' if ok else 'FAIL'}  {name}")
    if failed:
        print(f"ub-coverage-check --self-test: {len(failed)} FAILED")
        return 1
    print(f"ub-coverage-check --self-test: all {len(results)} passed")
    return 0


def main(argv=None):
    parser = argparse.ArgumentParser(description="UB disposition coverage checker.")
    parser.add_argument("--matrix", type=Path, default=DEFAULT_MATRIX)
    parser.add_argument("--schema", type=Path, default=DEFAULT_SCHEMA)
    parser.add_argument("--seed", action="store_true",
                        help="Check the seed set; PASS/FAIL as the final stdout line")
    parser.add_argument("--bucket", choices=sorted(BUCKET_AREAS),
                        help="Check one disposition area; PASS/FAIL")
    parser.add_argument("--strict", action="store_true",
                        help="Check every row AND completeness over the canonical UB classes")
    parser.add_argument("--summary", action="store_true",
                        help="With --strict: print a one-line coverage summary to stderr")
    parser.add_argument("--json", action="store_true",
                        help="Emit the full result as JSON to stdout")
    parser.add_argument("--warn-only", action="store_true",
                        help="Always exit 0 (bedding-in mode)")
    parser.add_argument("--self-test", action="store_true",
                        help="Run the built-in self-test and exit")
    args = parser.parse_args(argv)

    if args.self_test:
        return _self_test()

    try:
        from jsonschema import Draft202012Validator  # noqa: F401
    except ImportError:
        print("ub-coverage-check: jsonschema is required (pip install jsonschema)",
              file=sys.stderr)
        return 2

    dispositions = BUCKET_AREAS[args.bucket] if args.bucket else None
    try:
        result = run_check(args.matrix, args.schema,
                           dispositions=dispositions, strict=args.strict)
    except FileNotFoundError as exc:
        print(f"ub-coverage-check: {exc}", file=sys.stderr)
        return 2
    except ValueError as exc:
        print(f"ub-coverage-check: invalid JSON: {exc}", file=sys.stderr)
        return 2

    # A probe mode (--seed / --bucket / --strict) emits a single PASS/FAIL token
    # as the final stdout line; the full report goes to stderr.
    probe_mode = args.seed or args.bucket is not None or args.strict
    if args.json:
        print(json.dumps(result, indent=2))
    elif probe_mode:
        sys.stderr.write(format_text(result))
        if args.strict and args.summary:
            sys.stderr.write(_coverage_summary(result) + "\n")
        print("PASS" if result["green"] else "FAIL")
    else:
        sys.stderr.write(format_text(result))

    if args.warn_only:
        return 0
    return 0 if result["green"] else 1


if __name__ == "__main__":
    sys.exit(main())
