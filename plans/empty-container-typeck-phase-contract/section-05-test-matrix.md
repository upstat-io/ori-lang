---
section: "05"
title: "Test Matrix + Semantic Pins"
status: not-started
reviewed: false
goal: >
  Establish the full TDD scaffold — failing matrix tests, semantic pins, negative pins,
  and dual-execution parity verification — BEFORE any implementation lands. Every test
  authors the target behavior first; tests transition from "fails with today's codegen
  error" → "fails with target E2005 message" → "passes when annotated" as Sections
  01–04 land.
success_criteria:
  - "Test matrix covers ALL declared B × C × D cells — verified by the 05.4 audit checklist."
  - "Semantic pin `test_let_polymorphism_for_lambda` passes before and after Section 01 lands; reverting Section 01 must break it."
  - "Semantic pin `test_empty_list_emits_e2005_not_codegen_error` passes only after Section 03 is correctly integrated — reverting Section 03 must break it."
  - "Three negative pins reject broken behaviors — verified by attempting each broken state and confirming pin failure."
  - "Dual-execution parity: annotated empty-list programs produce identical output via `ori run` and `ori build` + exec, verified by `diagnostics/dual-exec-verify.sh` on ≥3 programs."
  - "All spec tests in `tests/spec/types/collections/empty_list/` round-trip via `cargo st` with the expected `#compile_fail(code: \"E2005\")` annotations AFTER Sections 01–03 land."
  - "AOT tests in `compiler/ori_llvm/tests/aot/empty_list.rs` pass via `timeout 150 cargo test -p ori_llvm` AFTER Sections 01–04 land."
inspired_by:
  - "Rust `compiletest` with `#[ui]` error-snapshot tests — pinning exact diagnostic codes and messages on #compile_fail cases; the `tests/ui/` suite uses one file per error scenario, exactly the pattern used in 05.2."
  - "Swift `test/SILOptimizer` feature matrix — dense type × pattern × phase cells ensure every optimization assumption is triangulated from multiple angles; corresponds to CLAUDE.md §Matrix Squeeze Principle."
  - "Roc property-test-derived regression suites — test coverage grows organically from the matrix; Roc's `tests/` corpus tracks exactly this `positive-file + negative-#compile_fail` pairing per feature (tests.md §Negative Testing Protocol)."
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "05.1"
    title: "Rust unit tests — validator + Value Restriction"
    status: not-started
  - id: "05.2"
    title: "Ori spec test corpus — 12+ files in tests/spec/types/collections/empty_list/"
    status: not-started
    # Note: 05.2 interaction tests total 18 files (items 11–17 + 16a); item 16a is the
    # negative-pin companion for trait-bounds interaction (split from item 16 which is
    # positive-only).
  - id: "05.3"
    title: "AOT integration tests + dual-execution parity"
    status: not-started
  - id: "05.4"
    title: "Matrix completeness audit"
    status: not-started
  - id: "05.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "05.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 05: Test Matrix + Semantic Pins

**Status:** Not Started
**Phase in implementation sequence:** Phase 0 — written FIRST (TDD). All tests are
authored as failing stubs and transitioned to passing as Sections 01–04 land.

**Mission.** Every fix that touches a code path shared by multiple types or patterns
requires matrix coverage (`tests.md §Matrix Testing Rule`, `CLAUDE.md §TDD for Bugs`).
This section builds the coverage lattice before any code changes so the matrix squeeze
principle (`CLAUDE.md §Matrix Squeeze Principle`) is in effect from day one: dense
pre-existing tests force the correct fix to thread precisely between passing and failing
cells and reveal the exact contract boundary.

## Context

`CLAUDE.md §TDD for Bugs` mandates: write the matrix tests first, verify they fail with
the current broken behavior, then fix, then verify they pass unchanged. This section
fulfills that mandate for the empty-container plan. The tests fall into three categories:

1. **Already-failing tests** — programs that currently fail with "unresolved type variable
   at codegen" (today's broken surface). They will transition to: (a) failing with E2005
   at typeck after Section 03 lands, and (b) passing (when annotated) after Sections
   01–03 are complete. These are tracked in the "Known Failing Tests" table below.

2. **Already-passing tests** — programs that work today and MUST keep working after the
   fix (let-polymorphism for lambdas, annotated empty lists, non-empty lists). Any
   regression here is a bug in the implementation.

3. **New tests** — programs testing behavior that is currently neither tested nor
   enforced. They document the target contract.

## Reference Implementations

- **Rust `rustc_hir_typeck`** — `rustc`'s existing `tests/ui/` suite uses one `.rs`
  file per error scenario with `//~ ERROR E0XXX` annotations. Section 05's spec test
  files mirror this discipline: one `.ori` file per scenario, `#compile_fail(code: "E2005")`
  for rejection cases. Rust's approach informed the file naming convention and the
  positive + negative pairing requirement (`tests.md §Negative Testing Protocol`).

- **Swift `test/SILOptimizer/`** — Swift's optimizer test suite uses a dense
  type × pattern matrix. Every optimization assumption is triangulated: there is a test
  where the optimization fires AND a test where it is blocked. Section 05's matrix
  dimensions follow this model: annotated (fires) vs unannotated (fires E2005) vs
  unannotated-with-constraint (post-fix, fires clean unification) form a three-cell
  column that pins the exact boundary.

- **Roc `tests/` corpus** — Roc's regression files grow organically as the compiler
  evolves. Section 05 seeds the corpus deliberately using the B × C × D matrix so
  growth is structured rather than ad-hoc.

**Depends on:** None. Tests are written before any implementation section.

---

## 05.1 Rust Unit Tests — Validator + Value Restriction

This subsection specifies the Rust unit tests that must be authored as part of the TDD
scaffold. They reside in two files corresponding to the modules each tests.

### 05.1.1 Value Restriction unit tests

**File:** `compiler/ori_types/src/infer/expr/tests.rs`
(sibling file per `compiler.md §Testing` — `blocks.rs` and `sequences.rs` are flat
files, not module directories, so their tests live in the parent directory's `tests.rs`,
NOT in `blocks/tests.rs` or `sequences/tests.rs`)

These tests are also referenced by Section 01 as the mandatory TDD stubs that
must exist BEFORE Section 01's implementation begins (per `section-01-value-restriction.md §01.1`).

**Test 1 — `test_let_polymorphism_for_lambda`** (SEMANTIC PIN)

Verifies that `let id = x -> x` produces a `Tag::Scheme` (the element type IS
generalized) and that `id` can be applied at both `int` and `str` in the same block.
This test MUST pass before and after the Value Restriction change. Reverting
`should_generalize` from Section 01 must break this test.

_Expected state across phases:_
- Phase 0 (today): PASSES — current behavior correctly generalizes lambda bindings
- Phase 1 (Section 01): PASSES — `should_generalize(ExprKind::Lambda) = true` preserves this

**Test 2 — `test_empty_list_let_binding_does_not_generalize_element_var`**

Verifies that after Section 01, the element type of `let xs = []` is NOT wrapped in a
`Tag::Scheme` — the element Var stays Unbound so the validator can surface E2005.

_Expected state:_
- Phase 0 (today): FAILS — current code unconditionally generalizes, wrapping in Scheme
- Phase 1 (Section 01): PASSES — `should_generalize([]) = false` keeps the Var Unbound

**Test 3 — `test_let_expr_non_lambda_does_not_generalize`**

Verifies the `infer_let` path (ExprKind::Let dispatch through `mod.rs`) for a
non-lambda initializer. Required by Section 01.3 as its dedicated test stub.

_Expected state:_ Phase 0 FAILS, Phase 1 PASSES.

**Test 4 — `test_try_block_let_non_lambda_does_not_generalize`**

Verifies the `sequences.rs` try-block let site. Required by Section 01.4.
Lives in `compiler/ori_types/src/infer/expr/tests.rs` (same file — `sequences.rs` is
a flat file; tests live in the parent directory's `tests.rs`, not `sequences/tests.rs`).

_Expected state:_ Phase 0 FAILS, Phase 1 PASSES.

### 05.1.2 Validator module unit tests

**File:** `compiler/ori_types/src/check/validators/tests.rs`
(created as part of Section 02; Section 05 cross-references the cells here for
matrix-completeness accounting)

The five validator cells specified in `section-02-validator-module.md §02.4`:

| Cell | Test name | Expected |
|------|-----------|----------|
| T1 | `validate_body_types_with_unbound_var_emits_ambiguous_type` | 1 E2005 error |
| T2 | `validate_body_types_with_resolved_int_produces_no_errors` | 0 errors (TF-5 gate) |
| T3 | `validate_body_types_with_error_type_produces_no_errors` | 0 errors (cascade suppression) |
| T4 | `validate_body_types_with_var_bound_inside_scheme_produces_no_errors` | 0 errors (bound var) |
| T5 | `validate_body_types_with_outer_var_inside_scheme_emits_ambiguous_type` | 1 E2005 error |

### 05.1.3 Bodies-pass integration site unit tests

**File:** `compiler/ori_types/src/check/bodies/tests.rs`
(sibling of `bodies/mod.rs` — tests live in `bodies/tests.rs` per compiler.md §Testing
since `bodies/mod.rs` is a module directory)

Section 03 wires `validate_body_types` into 4 call sites:
`check_function`, `check_test`, `check_impl_method`, `check_def_impl_method`.
Each site must have explicit test coverage so the matrix is complete.

| Cell | Test name | Site | Expected |
|------|-----------|------|----------|
| B1 | `validate_body_types_wired_in_check_function` | `check_function_bodies` | E2005 from top-level function with unannotated empty list |
| B2 | `validate_body_types_wired_in_check_test` | `check_test_bodies` | E2005 from `@test` body with unannotated empty list |
| B3 | `validate_body_types_wired_in_check_impl_method` | `check_impl_bodies` | E2005 from impl method body with unannotated empty list |
| B4 | `validate_body_types_wired_in_check_def_impl_method` | `check_def_impl_bodies` | E2005 from `def impl` method body with unannotated empty list |

These 4 cells prove all four Section 03 integration sites are wired. Without them, a
missing `validate_body_types` call in any site would be undetected by the test matrix.

### 05.1.4 Semantic + negative pins as Rust tests

**File:** `compiler/ori_types/src/check/validators/tests.rs` (additional cells)

**Pin SP-1 — `test_empty_list_emits_e2005_not_codegen_error`** (SEMANTIC PIN)

An integration test that drives the full type-checker pipeline on the original BUG-04-074
repro and asserts BOTH conditions:
1. `diagnostics` contains exactly one `TypeErrorKind::AmbiguousType` (E2005)
2. No codegen error (no "unresolved type variable at codegen" path fires)

This pin passes only when Sections 01 + 03 are both correctly implemented.
Reverting either section must break this test.

_Expected state:_ Phase 0 FAILS, Phase 3 PASSES.

**Pin SP-2 — `test_has_error_type_does_not_cascade_into_e2005`** (SEMANTIC PIN)

When an unrelated type error is already present in the same body, the validator's
HAS_ERROR cascade suppression (types.md §TK-3) must prevent a spurious E2005 from
appearing. This pin verifies that error recovery is monotone (typeck.md §ER-2) and
that the fix does not generate false positives on already-erroring programs.

_Expected state:_ Should PASS in Phase 0 (validator not yet wired) and remain PASSING
through all phases — this is a negative test for false-positive E2005.

**Pin NP-1 — `test_unannotated_empty_list_with_len_is_rejected_at_typeck`** (NEGATIVE PIN)

Verifies that the unannotated `let x = []; x.len()` program is REJECTED at the typeck
boundary (not at codegen). The error must be E2005, not any codegen-level error code.
Ensures no `Tag::Var` reaches LLVM. This pin rejects the old broken behavior.

_Expected state:_ Phase 0 FAILS (wrong error code / wrong phase), Phase 3 PASSES.

**Pin NP-2 — `test_scheme_captured_var_still_flagged`** (NEGATIVE PIN)

Builds a synthetic `expr_types` map containing a `Tag::Scheme` whose body contains an
unbound outer `Tag::Var` (var_id NOT in the scheme's bound-vars list). Asserts that
`validate_body_types` emits E2005 for this case. Prevents a "fix" that skips Schemes
entirely (which would miss captured-outer-var violations).

_Expected state:_ Phase 0 FAILS (no validator), Phase 2 PASSES (validator exists).

**Pin NP-3 — `test_tag_based_heuristic_fails_bidirectional_unification`** (NEGATIVE PIN)

Directly implements the Gemini Round-1 TPR finding from the BUG-04-074 fix-section:
`matches!(tag, Function | Scheme)` as the generalization guard fails when the resolved
type is still `Tag::Var` on a conditional lambda `let f = if cond then (x -> x) else (y -> y)`.
This test authors a type scenario where this tag-based heuristic would incorrectly pass,
then asserts that the AST-based `should_generalize(ExprKind::Lambda)` approach correctly
handles it. This prevents regression to any tag-based policy in Section 01.

_Expected state:_ Must PASS after Section 01 with AST-based approach. Would FAIL if
someone reverts to a tag-based check.

---

## 05.2 Ori Spec Test Corpus

**Directory:** `tests/spec/types/collections/empty_list/` (NEW directory — create it)

All files follow the convention of existing spec tests in `tests/spec/types/`: one
`.ori` file per scenario, inline `#compile_fail(code: "E2005")` annotation for rejection
cases (using the `code:` parameter form for exact error-code matching — NOT the simple
string form which does message-substring matching only), top-level `@test_XXX tests @YYY`
functions with `assert_eq` from `std.testing` for positive cases.

The `#compile_fail` attribute uses named-parameter syntax:
- `#compile_fail(code: "E2005")` — pins exact error code E2005 (correct)
- `#compile_fail("E2005")` — matches the string "E2005" as a message substring (WRONG for code-pinning)

Source: `compiler/ori_parse/src/grammar/attr/compile_fail.rs` distinguishes the
simple string form (message matching) from the named `code:` parameter form (code
matching). `compiler/oric/src/test/error_matching.rs` enforces the split.

Existing `tests/spec/types/` layout (verified): files are flat `.ori` files with
`@test_XXX tests @YYY () -> void` patterns and `use std.testing { assert, assert_eq }`
imports.

### File inventory (12 files)

---

#### `empty_list_annotated_with_push.ori` — CORE REPRO (annotated, must compile)

**Expected:** Compiles; test passes.

```ori
// Spec: 14-expressions.md:1224-1228 — annotated empty list clears type ambiguity
// Regression: BUG-04-074 — verifies annotated path compiles clean through LLVM

use std.testing { assert_eq }

@test_annotated_empty_list_push tests @annotated_empty_list_push () -> void = {
    assert_eq(actual: annotated_empty_list_push(), expected: 1)
}

@annotated_empty_list_push () -> int = {
    let ages: [int] = [];
    ages = ages.push(value: 10);
    ages.len()
}
```

_State across phases:_ Phase 0 FAILS (codegen error); Phase 3 PASSES (annotation
prevents E2005; clean pipeline through LLVM).

---

#### `empty_list_bare_with_push_and_len.ori` — PRIMARY E2005 TARGET

**Expected:** `#compile_fail(code: "E2005")`

```ori
// Spec: 14-expressions.md:1224-1228 — empty list without type context is an error
// Regression: BUG-04-074 — previously reached codegen with unresolved Tag::Var
#compile_fail(code: "E2005")
@main () -> int = {
    let ages = [];
    ages = ages.push(value: 10);
    ages.len()
}
```

_State:_ Phase 0 FAILS (wrong error kind — codegen error, not E2005); Phase 3 PASSES.

---

#### `empty_list_bare_with_len_only.ori` — DIMENSION C: len-only (no element constraint)

**Expected:** `#compile_fail(code: "E2005")`

```ori
// No usage that constrains the element type — E2005 mandatory per spec.
#compile_fail(code: "E2005")
@main () -> int = {
    let x = [];
    x.len()
}
```

_State:_ Phase 0 FAILS (codegen error), Phase 3 PASSES.

---

#### `empty_list_bare_with_is_empty_only.ori` — DIMENSION C: is_empty-only

**Expected:** `#compile_fail(code: "E2005")`

```ori
// No element constraint: is_empty() doesn't constrain the element type.
#compile_fail(code: "E2005")
@main () -> int = {
    let x = [];
    if x.is_empty() then 0 else 1
}
```

_State:_ Phase 0 FAILS (codegen error), Phase 3 PASSES.

---

#### `empty_list_chained_constraint.ori` — TRANSITION TEST (DIMENSION C: iter-chain)

**Expected (post-fix):** Compiles; test passes.

This is the primary TRANSITION test. Post-fix, usage fully constrains the element type
through the iter chain, so no annotation is needed. The `collect` at the end informs the
element type as `int`.

```ori
use std.testing { assert_eq }

@test_chained_constraint tests @chained_constraint () -> void = {
    assert_eq(actual: chained_constraint(), expected: 2)
}

@chained_constraint () -> int = {
    let xs: [int] = [];
    xs = xs.push(value: 1);
    xs = xs.push(value: 2);
    let doubled = xs.iter().map(x -> x * 2).collect();
    doubled.len()
}
```

Note: explicit annotation is used to ensure the file compiles in Phase 0 before
constraint-propagation is implemented. A subsequent version without the annotation
exercises the pure constraint path.

_State:_ Phase 0 compiles (annotated form); Phase 3+ exercises constraint inference.

---

#### `empty_list_nested_let.ori` — DIMENSION C: nested let

**Expected:** `#compile_fail(code: "E2005")`

```ori
// Nested indirection: the outer list's element type is itself a list.
// No usage constrains either list's element type.
#compile_fail(code: "E2005")
@main () -> int = {
    let x: [[int]] = [];
    let y = [];
    y.len()
}
```

Tests that the validator catches the nested unannotated list `y` even when `x` is
annotated. Element-type ambiguity on `y` must still surface E2005.

---

#### `empty_list_try_block.ori` — DIMENSION C: try-block let (Section 01.4 migration site)

**Expected:** `#compile_fail(code: "E2005")`

```ori
// Exercises the sequences.rs try-block let-generalize site (Section 01.4).
#compile_fail(code: "E2005")
@try_fn () -> Result<int, str> = {
    try {
        let xs = [];
        Ok(xs.len())
    }
}

@main () -> int = {
    match try_fn() {
        Ok(n) -> n,
        Err(_) -> 1
    }
}
```

_State:_ Phase 0 FAILS (codegen error or wrong phase); Phase 3 PASSES (E2005 from
try-block let site).

---

#### `empty_list_immutable_binding.ori` — DIMENSION D: annotated immutable binding

**Expected:** Compiles; test passes.

```ori
use std.testing { assert }

@test_annotated_immutable_binding tests @annotated_immutable_binding () -> void = {
    assert(cond: annotated_immutable_binding())
}

@annotated_immutable_binding () -> bool = {
    let $ages: [int] = [];
    ages.is_empty()
}
```

Verifies that annotated immutable bindings with empty lists compile clean.

---

#### `empty_list_element_struct.ori` — DIMENSION B: struct element

**Expected:** Compiles; test passes.

```ori
use std.testing { assert_eq }

type Point = { x: int, y: int }

@test_struct_element_empty_list tests @struct_element_empty_list () -> void = {
    assert_eq(actual: struct_element_empty_list(), expected: 1)
}

@struct_element_empty_list () -> int = {
    let pts: [Point] = [];
    pts = pts.push(value: Point { x: 1, y: 2 });
    pts.len()
}
```

Exercises struct-typed element inference through the annotated path.

---

#### `empty_list_element_closure.ori` — DIMENSION B: closure element (let-polymorphism interaction)

**Expected:** Compiles; test passes.

```ori
use std.testing { assert_eq }

@test_closure_element_empty_list tests @closure_element_empty_list () -> void = {
    assert_eq(actual: closure_element_empty_list(), expected: 1)
}

@closure_element_empty_list () -> int = {
    let fns: [(int) -> int] = [];
    fns = fns.push(value: (x -> x + 1));
    fns.len()
}
```

Tests the closure-typed element case, which interacts with let-polymorphism. The closure
`x -> x + 1` is monomorphic here (not generalizable via the annotated list annotation).
Verifies that the fix does not break closure storage in lists.

---

#### `empty_list_element_option.ori` — DIMENSION B: Option<int> element

**Expected:** Compiles; test passes.

```ori
use std.testing { assert_eq }

@test_option_element_empty_list tests @option_element_empty_list () -> void = {
    assert_eq(actual: option_element_empty_list(), expected: 2)
}

@option_element_empty_list () -> int = {
    let opts: [Option<int>] = [];
    opts = opts.push(value: Some(1));
    opts = opts.push(value: None);
    opts.len()
}
```

---

#### `empty_list_with_for_yield.ori` — DIMENSION C: for-yield pattern

**Expected:** Compiles; test passes.

```ori
use std.testing { assert_eq }

@test_for_yield_empty_list tests @for_yield_empty_list () -> void = {
    assert_eq(actual: for_yield_empty_list(), expected: 0)
}

@for_yield_empty_list () -> int = {
    let xs: [int] = [];
    let doubled = for x in xs yield x * 2;
    doubled.len()
}
```

Exercises the for-yield path on an empty annotated list. The yield result is a `[int]`
with zero elements; `doubled.len() == 0` is the assertion.

---

### Phase transition summary

| File | Phase 0 state | Phase 3 state |
|------|---------------|---------------|
| `empty_list_annotated_with_push.ori` | FAILS (codegen error) | PASSES |
| `empty_list_bare_with_push_and_len.ori` | FAILS (wrong error) | PASSES (E2005) |
| `empty_list_bare_with_len_only.ori` | FAILS (wrong error) | PASSES (E2005) |
| `empty_list_bare_with_is_empty_only.ori` | FAILS (wrong error) | PASSES (E2005) |
| `empty_list_chained_constraint.ori` | PASSES (annotated) | PASSES |
| `empty_list_nested_let.ori` | FAILS (wrong error) | PASSES (E2005) |
| `empty_list_try_block.ori` | FAILS (wrong error) | PASSES (E2005) |
| `empty_list_immutable_binding.ori` | FAILS (codegen error) | PASSES |
| `empty_list_element_struct.ori` | FAILS (codegen error) | PASSES |
| `empty_list_element_closure.ori` | FAILS (codegen error) | PASSES |
| `empty_list_element_option.ori` | FAILS (codegen error) | PASSES |
| `empty_list_with_for_yield.ori` | FAILS (codegen error) | PASSES |

---

## 05.3 AOT Integration Tests + Dual-Execution Parity

### 05.3.1 New AOT test file

**File:** `compiler/ori_llvm/tests/aot/empty_list.rs` (NEW — follows `collections_ext.rs` pattern)

The file uses `include_str!("fixtures/empty_list/<name>.ori")` with fixture files in
`compiler/ori_llvm/tests/aot/fixtures/empty_list/`. The `assert_aot_success` utility
from `crate::util` compiles and runs the program, asserting exit 0.

**IMPORTANT: Register the new module in `compiler/ori_llvm/tests/aot/main.rs`.**
The AOT suite is driven through an explicit `pub mod` list in `main.rs`. Creating
`empty_list.rs` without registering it means the tests will never run. Add:
```rust
pub mod empty_list;
```
to `compiler/ori_llvm/tests/aot/main.rs` in alphabetical order (after `elem_dec_scope`,
before `enum_discriminant`).

```rust
//! Empty List AOT Integration Tests
//!
//! Tests that annotated empty list programs compile and run correctly through the
//! full AOT pipeline (typeck → canonicalize → ARC → AIMS → LLVM → link → exec).
//! Verifies Section 04's defense-in-depth assertions do NOT fire on correct programs.
//!
//! All tests require Sections 01–03 (typeck fix) AND Section 04 (codegen assertions)
//! to be complete before they will pass.

use crate::util::assert_aot_success;
```

**Test 1 — `test_annotated_empty_list_with_push_exits_zero`**

Compiles `annotated_push.ori` (annotated `[int]`, push + len check) via AOT
and asserts exit 0. This is the core AOT regression pin for BUG-04-074.

**Test 2 — `test_annotated_empty_list_with_multiple_pushes`**

Compiles a program with 3 pushes and a length assertion. Exercises the `push` builtin
accumulator pattern through AOT.

**Test 3 — `test_annotated_struct_element_empty_list`**

Compiles the `empty_list_element_struct.ori` scenario through AOT. Verifies that struct
elements in annotated lists are correctly laid out in the LLVM backend.

**Test 4 — `test_annotated_empty_list_debug_build_no_assertion_fire`**

This test is specifically for Section 04. It compiles an annotated program in debug mode
and verifies that the debug build exits 0 (no assertion fired). If Section 04's
`debug_assert!` fires on a correctly-annotated program, this test catches the regression.

(Name follows `<subject>_<scenario>_<expected>` shape per `impl-hygiene.md §Test Function
Naming` — no ephemeral identifiers like `section_04` in the function name; provenance
is in the `///` doc comment of the function.)

### 05.3.2 Fixture files

Create `compiler/ori_llvm/tests/aot/fixtures/empty_list/` with:

- `annotated_push.ori` — annotated `[int]`, single push + len check, exits 0
- `annotated_multi_push.ori` — annotated `[int]`, 3 pushes, length assertion, exits 0
- `annotated_struct_element.ori` — annotated `[Point]`, push struct + len, exits 0

Fixture content matches the spec test files from 05.2 but uses AOT-compatible entry point
format. Since these are `@main () -> int` programs, they are compatible directly.

### 05.3.3 Dual-execution parity

Per `CLAUDE.md §Fix Completeness`: "Interpreter and LLVM produce identical results for
all new tests."

Run `diagnostics/dual-exec-verify.sh` on ≥3 annotated programs:

```bash
diagnostics/dual-exec-verify.sh tests/spec/types/collections/empty_list/empty_list_annotated_with_push.ori
diagnostics/dual-exec-verify.sh tests/spec/types/collections/empty_list/empty_list_element_struct.ori
diagnostics/dual-exec-verify.sh tests/spec/types/collections/empty_list/empty_list_with_for_yield.ori
```

Each invocation must report identical exit code and output between `ori run` (interpreter)
and `ori build` + exec (AOT). Any divergence is a dual-execution parity bug.

**Gate:** All three invocations pass before the plan can be marked complete.

---

## 05.4 Matrix Completeness Audit

This subsection audits coverage of the B × C × D matrix. Every cell must have at least
one test; missing cells are future regressions (`CLAUDE.md §Stabilization Discipline`).

**Dimension B — element type:**
- `int` (push test, len test, iter-chain test)
- `str` — MISSING; add `empty_list_element_str.ori` (annotated `[str]`, push + len)
- `bool` — MISSING; add `empty_list_element_bool.ori` (annotated `[bool]`, push + is_empty)
- `struct` (element_struct test ✓)
- `closure` (element_closure test ✓)
- `Option<int>` (element_option test ✓)

**Dimension C — usage pattern:**
- `push + len` (bare_with_push_and_len ✓, annotated_with_push ✓)
- `push + iter` (chained_constraint ✓)
- `push + map` — MISSING; add `empty_list_push_map.ori` (annotated, map transform)
- `push + is_empty` (annotated: immutable_binding via is_empty ✓; bare: is_empty_only ✓)
- `len only` (bare_with_len_only ✓)
- `is_empty only` (bare_with_is_empty_only ✓)
- `iter.map.filter.collect` — MISSING; add `empty_list_iter_chain.ori`
- `nested let` (nested_let ✓)
- `try block` (try_block ✓)
- `for yield` (with_for_yield ✓)

**Dimension D — constraint availability:**
- With explicit annotation (annotated_with_push ✓, immutable_binding ✓, element_struct ✓, element_closure ✓, element_option ✓, for_yield ✓)
- Without annotation + no usage constraint (bare_with_len_only ✓, bare_with_is_empty_only ✓, nested_let ✓)
- Without annotation + usage constrains (bare_with_push_and_len ✓ — E2005 pre-fix)

### Bodies-pass integration site coverage (Section 03 completeness)

Per the bodies-pass integration surface (Section 03), `validate_body_types` is wired into
four call sites. The matrix must include explicit positive and negative coverage for each:

| Site | Coverage file | Type |
|------|---------------|------|
| `check_function` | `empty_list_bare_with_push_and_len.ori` (`#compile_fail(code: "E2005")`) | negative |
| `check_function` | `empty_list_annotated_with_push.ori` (passes) | positive |
| `check_test` | `empty_list_unannotated_in_test.ori` (`#compile_fail(code: "E2005")`) | negative — MISSING |
| `check_test` | `empty_list_annotated_in_test.ori` (passes) | positive — MISSING |
| `check_impl_method` | `empty_list_unannotated_in_impl.ori` (`#compile_fail(code: "E2005")`) | negative — MISSING |
| `check_impl_method` | `empty_list_annotated_in_impl.ori` (passes) | positive — MISSING |
| `check_def_impl_method` | `empty_list_unannotated_in_def_impl.ori` (`#compile_fail(code: "E2005")`) | negative — MISSING |
| `check_def_impl_method` | `empty_list_annotated_in_def_impl.ori` (passes) | positive — MISSING |

**Required additions — 6 spec test files for bodies-pass integration coverage:**

5. **`empty_list_unannotated_in_test.ori`** — unannotated `let xs = []` inside an
   `@test` body. Expected: `#compile_fail(code: "E2005")`. Exercises `check_test_bodies`.

6. **`empty_list_annotated_in_test.ori`** — annotated `let xs: [int] = []` inside an
   `@test` body. Expected: compiles and passes. Positive pin for `check_test_bodies`.

7. **`empty_list_unannotated_in_impl.ori`** — unannotated `let xs = []` inside a method
   body (`impl Type { @method }`). Expected: `#compile_fail(code: "E2005")`. Exercises
   `check_impl_bodies`.

8. **`empty_list_annotated_in_impl.ori`** — annotated `let xs: [int] = []` inside an
   impl method body. Expected: compiles clean. Positive pin for `check_impl_bodies`.

9. **`empty_list_unannotated_in_def_impl.ori`** — unannotated `let xs = []` inside a
   `def impl` method body. Expected: `#compile_fail(code: "E2005")`. Exercises
   `check_def_impl_bodies`.

10. **`empty_list_annotated_in_def_impl.ori`** — annotated `let xs: [int] = []` inside
    a `def impl` method body. Expected: compiles clean. Positive pin for
    `check_def_impl_bodies`.

### Interaction testing (tests.md §Interaction Testing MANDATORY)

Per `tests.md §Interaction Testing`, type inference changes must be tested with:
"Generics, closures, trait bounds, `?` operator, pattern matching."

Required additions — 5 interaction spec test files:

11. **`empty_list_pattern_match_interaction.ori`** — empty list in `match` arm scrutinee
    position: `match empty_list { [] -> ..., [x, ..rest] -> ... }`. Expected: compiles
    clean with annotation. Exercises type inference × pattern matching.

12. **`empty_list_generic_function_interaction.ori`** — empty list passed to a generic
    function `@take_list<T> (xs: [T]) -> int = xs.len()`. Annotated `[int]` form
    compiles; unannotated form emits `#compile_fail(code: "E2005")`. Exercises type
    inference × generics.

13. **`empty_list_closure_capture_interaction.ori`** — closure that captures an annotated
    empty list and pushes into it: `let xs: [int] = []; let push_fn = v -> xs.push(value: v)`.
    Expected: compiles clean. Exercises type inference × closures.

14. **`empty_list_question_mark_interaction.ori`** — `let xs: [Result<int, str>] = []` in
    a `try` block with `?` propagation on elements. Expected: compiles clean. Exercises
    type inference × `?` operator.

15. **`empty_list_unannotated_generic_interaction.ori`** — unannotated empty list passed
    to a generic function without constraint propagation. Expected:
    `#compile_fail(code: "E2005")`. Exercises the negative pin for generics interaction.

16. **`empty_list_trait_bound_interaction.ori`** — empty list used where element type is
    constrained by a trait bound: `@process<T: Printable> (xs: [T]) -> int = xs.len()`.
    Annotated `let xs: [str] = []` passed to `process` must compile clean.
    Expected: no compile error. Exercises the positive pin for type inference × trait bounds.

    **Note**: A single `.ori` file cannot be both a compile-clean test and a
    `#compile_fail` test — `#compile_fail` is a file-level attribute. The negative pin
    lives in a companion file (item 16a).

16a. **`empty_list_unannotated_trait_bound_interaction.ori`** — unannotated empty list
    passed to a trait-bound generic function without annotation context:
    `let xs = []; process(xs: xs)` where `@process<T: Printable>`.
    Expected: `#compile_fail(code: "E2005")`. Exercises the negative pin for type
    inference × trait bounds (companion to item 16).

### Fault tolerance (tests.md §Cross-Phase Verification #3 MANDATORY)

Per `tests.md §Cross-Phase Verification` fault tolerance rule: "Write multi-error
`#compile_fail` tests" to verify ALL errors are reported, not just the first.

Required addition — 1 fault-tolerance spec test file:

17. **`empty_list_multiple_unannotated.ori`** — multiple unannotated empty lists in the
    same body: `let xs = []; let ys = []; xs.len() + ys.len()`. Expected:
    `#compile_fail(code: "E2005")`. Verifies the validator reports both errors and does
    not bail after the first. If the validator bails after the first error, the test still
    passes (one E2005 matches the code pin) — to confirm BOTH are reported, use a Rust
    integration test (`validate_body_types_emits_one_e2005_per_unbound_var` in
    `check/validators/tests.rs`) that asserts `diagnostics.len() == 2`.

### Missing cells — required additions before 05.N close-out

The following additional spec test files MUST be created to satisfy matrix completeness:

1. **`empty_list_element_str.ori`** — annotated `[str]`, push `"hello"`, len check.
   Uses `@test` + `assert_eq`. Fills B=str cell.

2. **`empty_list_element_bool.ori`** — annotated `[bool]`, push `true`, is_empty check.
   Uses `@test` + `assert_eq`. Fills B=bool cell.

3. **`empty_list_push_map.ori`** — annotated `[int]`, push + map with `x -> x * 2`.
   Uses `@test` + `assert_eq`. Fills C=push+map cell.

4. **`empty_list_iter_chain.ori`** — annotated `[int]`, push 3 values,
   `.iter().map().filter().collect()`, assert length. Uses `@test` + `assert_eq`.
   Fills C=iter.map.filter.collect cell.

Plus items 5–17 and item 16a from the bodies-pass and interaction testing sections above.
(Item 16a is the negative-pin companion for the trait-bounds interaction; it is separate
from the original 17-item count, bringing the total to 18 interaction/bodies-pass spec files.)

### Completeness audit checklist

- [ ] B=int: ≥1 test (annotated + bare) ✓
- [ ] B=str: ≥1 test — requires `empty_list_element_str.ori`
- [ ] B=bool: ≥1 test — requires `empty_list_element_bool.ori`
- [ ] B=struct: ≥1 test ✓
- [ ] B=closure: ≥1 test ✓
- [ ] B=Option<int>: ≥1 test ✓
- [ ] C=push+len: ≥1 annotated, ≥1 bare-compile-fail ✓
- [ ] C=push+iter: ≥1 test ✓
- [ ] C=push+map: ≥1 test — requires `empty_list_push_map.ori`
- [ ] C=push+is_empty: ≥1 test ✓
- [ ] C=len-only: ≥1 compile-fail ✓
- [ ] C=is_empty-only: ≥1 compile-fail ✓
- [ ] C=iter.map.filter.collect: ≥1 test — requires `empty_list_iter_chain.ori`
- [ ] C=nested-let: ≥1 compile-fail ✓
- [ ] C=try-block: ≥1 compile-fail ✓
- [ ] C=for-yield: ≥1 test ✓
- [ ] D=annotated: ≥1 test per B-type ✓ (int, struct, closure, option covered)
- [ ] D=no-annotation-no-constraint: ≥1 compile-fail ✓
- [ ] D=no-annotation-usage-constrains: ≥1 test (E2005 pre-fix, passes after fix) ✓
- [ ] Bodies-pass site check_function: positive + negative ✓ (annotated_with_push + bare_with_push_and_len)
- [ ] Bodies-pass site check_test: positive + negative — requires items 5–6
- [ ] Bodies-pass site check_impl_method: positive + negative — requires items 7–8
- [ ] Bodies-pass site check_def_impl_method: positive + negative — requires items 9–10
- [ ] Interaction: type inference × pattern matching — requires item 11
- [ ] Interaction: type inference × generics — requires items 12 + 15
- [ ] Interaction: type inference × closures — requires item 13
- [ ] Interaction: type inference × `?` operator — requires item 14
- [ ] Interaction: type inference × trait bounds — requires items 16 + 16a
- [ ] Fault tolerance: multi-error compile_fail — requires item 17
- [ ] Semantic pin SP-1 (`test_let_polymorphism_for_lambda`) ✓
- [ ] Semantic pin SP-2 (`test_empty_list_emits_e2005_not_codegen_error`) ✓
- [ ] Negative pin NP-1 (`test_unannotated_empty_list_with_len_is_rejected_at_typeck`) ✓
- [ ] Negative pin NP-2 (`test_scheme_captured_var_still_flagged`) ✓
- [ ] Negative pin NP-3 (`test_tag_based_heuristic_fails_bidirectional_unification`) ✓
- [ ] AOT tests: ≥4 tests in `empty_list.rs` ✓
- [ ] AOT module registered: `pub mod empty_list;` added to `compiler/ori_llvm/tests/aot/main.rs`
- [ ] Dual-execution parity: ≥3 `dual-exec-verify.sh` programs documented ✓

---

## Known Failing Tests

These tests are EXPECTED to fail until the specified phase lands. Do NOT investigate
them as separate bugs; do NOT rewrite them to avoid failures. Their failing state is the
deliverable (`CLAUDE.md §OWNERSHIP — Tests that expose bugs = bugs found`).

| Test | Expected failure today | Passes after | Root cause |
|------|------------------------|-------------|------------|
| `empty_list_bare_with_push_and_len.ori` | "unresolved type variable at codegen" | Phase 3 (Section 03) | Validator not yet wired into bodies pass |
| `empty_list_bare_with_len_only.ori` | codegen error | Phase 3 | Same |
| `empty_list_bare_with_is_empty_only.ori` | codegen error | Phase 3 | Same |
| `empty_list_nested_let.ori` | codegen error | Phase 3 | Same |
| `empty_list_try_block.ori` | codegen error | Phase 3 | Same |
| `empty_list_annotated_with_push.ori` | codegen error | Phase 3 | Tag::Var reaches codegen; Section 01 stops generalization |
| `empty_list_immutable_binding.ori` | codegen error | Phase 3 | Same |
| `empty_list_element_struct.ori` | codegen error | Phase 3 | Same |
| `empty_list_element_closure.ori` | codegen error | Phase 3 | Same |
| `empty_list_element_option.ori` | codegen error | Phase 3 | Same |
| `empty_list_with_for_yield.ori` | codegen error | Phase 3 | Same |
| `test_empty_list_let_binding_does_not_generalize_element_var` | FAILS (element IS generalized today) | Phase 1 (Section 01) | should_generalize not yet extracted |
| `test_empty_list_emits_e2005_not_codegen_error` (Rust test) | FAILS (wrong error kind) | Phase 3 (Section 03) | Validator not wired into bodies pass |
| AOT tests in `empty_list.rs` | FAIL (codegen error) | Phase 4 (Section 04) | Depends on full producer+consumer pipeline |

`test_let_polymorphism_for_lambda` — PASSES in Phase 0; may transiently FAIL during
Section 01 implementation if `should_generalize` is too narrowly scoped. This is a
semantic pin; a transient failure during 01.1 implementation is expected.

---

## 05.R Third Party Review Findings

Round 1 — Dual-source TPR on test scaffold (Codex + Gemini). All 11 findings
fixed in this revision.

### [[TPR-05-001-codex]] [HIGH] Replace message-form compile_fail pins with code-based expectations

**Location:** All `#compile_fail` annotations throughout 05.2 and 05.4
**Reviewer:** Codex | **Status:** Fixed

**Evidence:** `compiler/ori_parse/src/grammar/attr/compile_fail.rs:3-4 and :16-19`
distinguish simple string form as message matching; `compiler/oric/src/test/error_matching.rs:81-95`
enforces the split. `#compile_fail("E2005")` does NOT pin the error code — it matches
the string "E2005" as a message substring.

**Fix:** All compile_fail annotations now use `#compile_fail(code: "E2005")` (named
`code:` parameter form) to pin the exact error code. The simple string form is reserved
for message-substring matching only.

---

### [[TPR-05-002-codex]] [HIGH] Expand matrix to cover all four bodies-pass integration sites

**Location:** `section-05-test-matrix.md` 05.4
**Reviewer:** Codex | **Status:** Fixed

**Evidence:** Section 03 wires `validate_body_types` into 4 bodies-pass exits:
`check_function`, `check_test`, `check_impl_method`, `check_def_impl_method`
(per `section-03-bodies-pass-integration.md:7-10`). The original 05.4 matrix
covered only the `check_function` path.

**Fix:** Added explicit positive and negative coverage for `check_test`, `check_impl_method`,
and `check_def_impl_method` (items 5–10 in the missing-cells section). Bodies-pass
site coverage rows added to the completeness audit checklist. Bodies-pass integration
unit tests (B1–B4) added to 05.1.3.

---

### [[TPR-05-003-codex]] [MEDIUM] Register the new empty_list AOT module in the test harness

**Location:** `compiler/ori_llvm/tests/aot/main.rs`
**Reviewer:** Codex | **Status:** Fixed

**Evidence:** `compiler/ori_llvm/tests/aot/main.rs` drives the AOT suite through an
explicit `pub mod` list. `empty_list.rs` was not registered; the tests would never run.

**Fix:** Added explicit instruction in 05.3.1 to add `pub mod empty_list;` to
`compiler/ori_llvm/tests/aot/main.rs` in alphabetical order. Marked as a required step
in the AOT test creation workflow.

---

### [[TPR-05-004-codex]] [MEDIUM] Wire planned infer expr test files into the Rust module tree

**Location:** `section-05-test-matrix.md` 05.1.1
**Reviewer:** Codex (overlaps with TPR-05-001-gemini) | **Status:** Fixed

**Evidence:** `compiler/ori_types/src/infer/expr/blocks.rs` and `sequences.rs` are
flat files (not module directories). No `mod tests;` declaration exists in either file.
The planned `blocks/tests.rs` and `sequences/tests.rs` directories do not exist and
cannot be created without adding explicit `mod tests;` declarations to the respective
`.rs` files. Per `compiler.md §Testing`: "flat file `foo.rs` → tests live in the same
directory's `tests.rs`."

**Fix:** Test locations updated to `compiler/ori_types/src/infer/expr/tests.rs` (the
existing sibling test file for the `infer/expr/` directory) for both Test 3 and Test 4.

---

### [[TPR-05-005-codex]] [MEDIUM] Add the missing Section 06 and Section 07 plan files

**Location:** `plans/empty-container-typeck-phase-contract/index.md`
**Reviewer:** Codex | **Status:** Fixed (see below)

**Evidence:** `index.md` references `section-06-diagnostics-audit.md` and
`section-07-closeout.md` as concrete files. The plan directory contains only sections
01–05. The overview makes these part of the dependency graph.

**Fix:** Created `section-06-diagnostics-audit.md` and `section-07-closeout.md` as
proper stub plan section files with frontmatter, goals, and placeholder content.

---

### [[TPR-05-001-gemini]] [LOW] Correct unit test file paths for flat files

**Location:** `section-05-test-matrix.md` 05.1.1
**Reviewer:** Gemini (subsumes TPR-05-004-codex) | **Status:** Fixed

Same finding as TPR-05-004-codex. See that entry for the fix.

---

### [[TPR-05-002-gemini]] [LOW] Remove ephemeral ID from AOT test name

**Location:** `section-05-test-matrix.md` 05.3.1 Test 4
**Reviewer:** Gemini | **Status:** Fixed

**Evidence:** `test_section_04_debug_assert_does_not_fire_on_annotated_program` contains
the ephemeral `section_04` identifier, violating `impl-hygiene.md §Test Function Naming`
TDD-9: "No ephemeral identifiers (BUG-04-074, section-05) in function names."

**Fix:** Renamed to `test_annotated_empty_list_debug_build_no_assertion_fire` which
follows the `<subject>_<scenario>_<expected>` shape. BUG/section provenance goes in
the function's `///` doc comment.

---

### [[TPR-05-003-gemini]] [HIGH] Add explicit assertions to positive spec tests

**Location:** `section-05-test-matrix.md` 05.2
**Reviewer:** Gemini | **Status:** Fixed

**Evidence:** Original positive spec tests used `if condition then 0 else 1` exit-code
style. Per `tests.md §Test Hygiene` TDD-8: "No orphan tests: every test file must
contain at least one assertion." Exit-code programs prove nothing about correctness at
the spec-test level — they rely on the test harness comparing exit codes. The correct
pattern for `tests/spec/types/` is `@test_XXX tests @YYY () -> void` with `assert_eq`
from `std.testing`, as used throughout `tests/spec/types/list_types.ori`,
`lambda_mono.ori`, etc.

**Fix:** All positive spec test files now use `@test_XXX tests @YYY () -> void` with
`use std.testing { assert, assert_eq }` and explicit `assert_eq(actual: ..., expected: ...)`
assertions. The `@main () -> int` exit-code style is retained only for `#compile_fail`
test programs (which have no assertion body by definition) and AOT fixture files.

---

### [[TPR-05-004-gemini]] [HIGH] Add mandated interaction tests for type inference

**Location:** `section-05-test-matrix.md` 05.4
**Reviewer:** Gemini | **Status:** Fixed

**Evidence:** `tests.md §Interaction Testing` mandates: "Type inference → Also test with:
Generics, closures, trait bounds, `?` operator, pattern matching." The original 05.4
matrix lacked explicit interaction cells.

**Fix:** Added 5 interaction spec test files (items 11–15) covering:
- Type inference × pattern matching (`empty_list_pattern_match_interaction.ori`)
- Type inference × generics (positive: `empty_list_generic_function_interaction.ori`,
  negative: `empty_list_unannotated_generic_interaction.ori`)
- Type inference × closures (`empty_list_closure_capture_interaction.ori`)
- Type inference × `?` operator (`empty_list_question_mark_interaction.ori`)
Corresponding completeness checklist rows added.

---

### [[TPR-05-005-gemini]] [LOW] Add multi-error fault tolerance test for E2005

**Location:** `section-05-test-matrix.md` 05.4
**Reviewer:** Gemini | **Status:** Fixed

**Evidence:** `tests.md §Cross-Phase Verification #3`: "Write multi-error `#compile_fail`
tests" to verify ALL errors are reported. A single-error test cannot catch a validator
that bails after the first error.

**Fix:** Added item 16 (`empty_list_multiple_unannotated.ori`) with two unannotated
empty lists in the same body, plus a companion Rust unit test
`validate_body_types_emits_one_e2005_per_unbound_var` that asserts `diagnostics.len() == 2`.

---

### [[TPR-05-006-gemini]] [LOW] Fix main return type in is_empty_only spec test

**Location:** `section-05-test-matrix.md` 05.2 `empty_list_bare_with_is_empty_only.ori`
**Reviewer:** Gemini | **Status:** Fixed

**Evidence:** Original test used `@main () -> bool`. Per `CLAUDE.md §Entry Points`,
`@main` only supports `void`, `int`, or `Result` return types. `bool` produces a
type-check error ("expected int/void, found bool") before reaching the targeted E2005.

**Fix:** Changed to `@main () -> int = { let x = []; if x.is_empty() then 0 else 1 }`.
The test still exercises the unannotated empty list E2005 path while having a valid
`@main` return type.

---

Round 2 — Dual-source TPR on sections 05, 06, 07 (Codex + Gemini). Findings addressed
in this revision.

### [[TPR-05-R2-001-codex]] [MEDIUM] Add the missing trait-bounds interaction cell

**Location:** `plans/empty-container-typeck-phase-contract/section-05-test-matrix.md:721`
**Reviewer:** Codex | **Status:** Fixed

**Evidence:** The interaction test list cited `tests.md`'s mandatory interaction list
verbatim: "Generics, closures, trait bounds, `?` operator, pattern matching." The
required additions (items 11–15) scheduled pattern matching, generics, closures, and `?`,
but contained no bounded-generic or trait-constrained scenario. There was no
`T: Trait`-boundary test anywhere in the interaction list or the matrix audit.

**Fix:** Added item 16 (`empty_list_trait_bound_interaction.ori`) — a test where the
empty list element type is constrained by a `T: Printable` bound. Both a positive
(annotated `[str]` passed to the generic function) and a negative (`#compile_fail(code: "E2005")`
for unannotated form) are covered. Completeness audit checklist updated to include the
trait-bounds interaction row. 05.N updated from 16 to 17 total additional spec test files.

---

### [[TPR-05-R2-002-codex]] [LOW] Align the AOT scaffold with the real fixture and module inventory

**Location:** `plans/empty-container-typeck-phase-contract/section-05-test-matrix.md:596`
**Reviewer:** Codex | **Status:** Fixed

**Evidence:** Section 05 said Test 1 compiles `empty_list_annotated_push.ori`, but the
fixture inventory in the same subsection (05.3.2) defined `annotated_push.ori` — an
inconsistency. The registration instruction also said to add `pub mod empty_list;`
"after `double_ended`, before `expressions`", while `compiler/ori_llvm/tests/aot/main.rs`
contains neither neighbor module.

**Fix:** Canonicalized the fixture filename to `annotated_push.ori` throughout section
05.3.1 (matches the 05.3.2 fixture definition). Updated the registration instruction to
cite the real alphabetical neighbors: "after `elem_dec_scope`, before `enum_discriminant`"
(verified against `compiler/ori_llvm/tests/aot/main.rs`).

Round 3 — Dual-source TPR on sections 05, 06, 07 (Codex + Gemini). Findings addressed
in this revision.

### [[TPR-05-R3-001-codex]] [HIGH] Split trait-bounds interaction into separate positive and negative files

**Location:** `plans/empty-container-typeck-phase-contract/section-05-test-matrix.md:747`
**Reviewer:** Codex | **Status:** Fixed

**Evidence:** Item 16 specified a single `empty_list_trait_bound_interaction.ori` that both
"compiles clean" (for the annotated case) and "emits `#compile_fail(code: \"E2005\")`" (for
the unannotated case). Since `#compile_fail` is a file-level attribute, a single `.ori` file
cannot simultaneously be a passing test and a compile-fail test. The generics interaction
(item 12 + item 15) already demonstrated the correct split pattern — item 12 is positive-only
and item 15 is the negative companion. Item 16 failed to follow this pattern.

**Fix:** Kept item 16 as a positive-only file (`empty_list_trait_bound_interaction.ori`).
Added item 16a (`empty_list_unannotated_trait_bound_interaction.ori`) as the negative-pin
companion. Updated completeness audit checklist row to reference both items 16 and 16a.
Updated 05.N 05.4 checklist to name 18 total additional spec test files (was 17).

---

## 05.N Completion Checklist

This section is complete when ALL of the following are true. Note: Section 05 completion
(writing the scaffold) precedes implementation; the implementation gates (AOT tests
passing, spec tests passing) are verified at Section 07 close-out, not here.

- [ ] **05.1 complete** — 4 Rust unit test stubs authored in
  `compiler/ori_types/src/infer/expr/tests.rs`; 5 validator tests specified in
  `check/validators/tests.rs`; 4 bodies-pass integration site tests (B1–B4) specified
  in `check/bodies/tests.rs`; 3 semantic pins + 3 negative pins authored
- [ ] **05.2 complete** — 12 core spec test files created in
  `tests/spec/types/collections/empty_list/`; all positive tests use `@test` + `assert_eq`
  pattern; all compile_fail tests use `#compile_fail(code: "E2005")`
- [ ] **05.3 complete** — `compiler/ori_llvm/tests/aot/empty_list.rs` created with
  4 test stubs + 3 fixture files; `pub mod empty_list;` added to
  `compiler/ori_llvm/tests/aot/main.rs`; `dual-exec-verify.sh` invocation documented
- [ ] **05.4 complete** — matrix completeness audit checklist fully populated; 4 missing
  B/C cells identified; 6 bodies-pass integration coverage files identified; 6 interaction
  files identified (trait-bounds split into items 16 + 16a); 1 fault-tolerance file
  identified; all 18 additional spec test files
  (`element_str`, `element_bool`, `push_map`, `iter_chain`, `unannotated_in_test`,
  `annotated_in_test`, `unannotated_in_impl`, `annotated_in_impl`,
  `unannotated_in_def_impl`, `annotated_in_def_impl`, `pattern_match_interaction`,
  `generic_function_interaction`, `closure_capture_interaction`,
  `question_mark_interaction`, `unannotated_generic_interaction`,
  `trait_bound_interaction`, `unannotated_trait_bound_interaction`,
  `multiple_unannotated`) created to fill missing cells; all cells checked
- [ ] `timeout 150 cargo test -p ori_types` runs (some tests fail as expected by the
  Known Failing Tests table; no UNEXPECTED failures)
- [ ] `cargo st tests/spec/types/collections/empty_list/` runs (expected failures only)
- [ ] `/tpr-review` passed on the test scaffold — independent dual-source review
  (Codex + Gemini) clean or all findings triaged and recorded in 05.R. Run BEFORE
  any implementation section to catch gaps with a fresh perspective.
- [ ] `/impl-hygiene-review` passed — test naming follows `<subject>_<scenario>_<expected>`
  convention; no ephemeral identifiers in test function names; no `#[allow(clippy)]`
  without reason; all `///` doc comments reference BUG-04-074 provenance
- [ ] `/improve-tooling` sweep — verify the dual-exec-verify.sh and test-all.sh invocations
  are sufficient; flag any gaps in test infrastructure surfaced during scaffold authoring
- [ ] `/sync-claude` sweep — verify no new APIs or commands were introduced that need
  documenting; if `diagnostics/dual-exec-verify.sh` behavior is confirmed, document its
  exact invocation pattern in CLAUDE.md §Commands if not already present

**Exit criteria:** Test scaffold is complete. Every matrix cell has ≥1 test or a tracked
missing-cell entry. Semantic and negative pins are authored with the correct expected
behavior. Section 01 may begin immediately after 05.N closes.
