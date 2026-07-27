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
The matrix is complete — all 39 canonical UB classes are dispositioned — so
`--strict --summary` reads green (39/39).

## What a green gate certifies — and what it does NOT

A green `--strict` gate certifies exactly this: **every canonical UB class
carries a disposition row, every foreclosure/obligation cites a pin, and every
gap cites a hardening anchor.** It does NOT certify that every foreclosure is
*discharged*.

The checker's pin resolution proves a pin **exists** (the file, or `file::symbol`,
is present), not that it **passes**. This matters most for `aims-obligation`
rows: they pin into the AIMS realization surface, whose RC/AOT verdict comes
from the verifier (`ORI_VERIFY_ARC=1 ORI_VERIFY_EACH=1`) plus a leak check, not
from any single build. A green matrix therefore means "the safety frontier is
mapped and every claim is anchored," never "every claim is proven discharged."
Read an `aims-obligation` row as an *obligation declared and pinned*, and run its
pin's real verdict surface to confirm discharge.

## Regression gate

`ub-coverage-check.py --strict --self-test` is the regression gate. It fails the
moment a foreclosure loses its pin, a pin stops resolving, or a new UB class
appears undispositioned — keeping the safety frontier honest as the compiler
evolves. Run it after any change that touches the UB surface (a new spec clause,
a new FFI form, a removed test that was a pin). Maintainers add a new row when
upstream `UndefinedBehaviorInfo` / miri grows a class; the gate's `--strict`
completeness check surfaces the gap.
