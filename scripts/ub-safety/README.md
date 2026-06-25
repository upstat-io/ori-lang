# UB safety coverage matrix

This directory holds Ori's undefined-behavior (UB) coverage matrix: a row per
miri-enumerated UB class recording **where the proof that the class cannot
occur stops**, and the pin that backs that claim. It is the safety-side mirror
of the runtime-panic catalogue (spec Clause 23.5); spec Clause 4.5 requires
that Ori programs have no silent undefined behavior.

The authoritative UB class set is rustc's `UndefinedBehaviorInfo` (37 variants)
+ `ValidationErrorKind` (26 validity sub-rules) at
`rustc_middle/src/mir/interpret/error.rs`, plus the miri runtime detectors
(data race, weak memory) that catch classes the type system permits. Most of
those classes are *foreclosed at compile time* in Ori — there is no raw
pointer dereference, no uninitialized binding, no transmute, and uninhabited
types (`Never`) are unconstructable. The matrix makes that claim explicit and
checkable for every class.

## Files

| File | Role |
|---|---|
| `coverage-matrix.schema.json` | JSON Schema (Draft 2020-12). Source of truth for the disposition taxonomy and the row shape. |
| `coverage-matrix.json` | The matrix data: one row per UB class. |
| `../ub-coverage-check.py` | The coverage checker. Validates the matrix, verifies every row is grounded and pinned, and measures completeness. |

## Disposition taxonomy

Each row carries exactly one disposition — the answer to "where does the proof
that this UB class cannot occur stop?":

| Disposition | Meaning |
|---|---|
| `foreclosed-typesystem` | The type system makes the UB unrepresentable (e.g. `Never` is uninhabited; no invalid bit patterns). |
| `aims-obligation` | Foreclosed at the type level, but realizing it correctly is an obligation discharged at codegen/runtime by AIMS (e.g. alignment), pinned by a test. |
| `data-race-foreclosed` | Value semantics plus the `Sendable` marker forbid the data race; no shared mutable state can be aliased across threads. |
| `ffi-unsafe-boundary` | Foreclosure stops at the FFI boundary; the class is checked at that boundary rather than proven away. |
| `gap` | Neither foreclosed nor obligated yet. A gap row **must** cite a hardening anchor; the checker fails closed without one. |

## Row shape

Every row records:

- `class_id` — the rustc `UndefinedBehaviorInfo` / `ValidationErrorKind` variant, or a miri detector class.
- `miri_source` — the upstream enumeration source (repo-relative).
- `disposition` — one of the taxonomy values above.
- `ori_mechanism` — the Ori invariant/mechanism and spec clause that dispositions the class.
- `pin_ref` — a `compiler_repo`-relative file (or `file::symbol`) that proves the foreclosure, anchors the obligation, or cites the gap's hardening anchor.

## Running the checker

```sh
python3 scripts/ub-coverage-check.py                 # full row-per-class report
python3 scripts/ub-coverage-check.py --seed          # seed-set verdict (PASS/FAIL)
python3 scripts/ub-coverage-check.py --bucket aims-obligation   # one disposition area
python3 scripts/ub-coverage-check.py --strict --summary         # completeness gate
python3 scripts/ub-coverage-check.py --json          # machine-readable report
python3 scripts/ub-coverage-check.py --self-test
```

A row is green only when the matrix is schema-valid, the row carries a non-empty
pin, and the pin resolves to a real file on disk. A `gap` row with no anchor, an
unpinned foreclosure, or an unresolvable pin reads red.

`--strict` additionally requires that every canonical UB class carries a row.
The matrix currently seeds one row per disposition bucket, so `--strict` reads
red (incomplete) and `--seed` reads green; the per-class population grows as
each disposition area is worked.
