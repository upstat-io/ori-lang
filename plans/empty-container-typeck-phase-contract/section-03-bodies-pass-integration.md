---
section: "03"
title: "Bodies-Pass Integration"
status: not-started
reviewed: false
goal: >
  Wire validate_body_types() (from Section 02) into all 4 bodies-pass call sites
  per typeck.md CK-1 — check_function, check_test, check_impl_method, and
  check_def_impl_method — so that every body-checked function produces E2005 for
  surviving unbound Tag::Var rather than passing them silently to codegen.
success_criteria:
  - "All 4 bodies-pass call sites invoke validate_body_types — verified by `grep -c 'validate_body_types' compiler/ori_types/src/check/bodies/mod.rs` returning 4."
  - "The original BUG-04-074 repro `@main () -> int = { let ages = []; ages = ages.push(value: 10); if ages.len() == 1 then 0 else 1 }` emits E2005 at typeck (not at codegen) — verified by a Rust integration test asserting on TypeErrorKind::AmbiguousType and the ABSENCE of any codegen error path firing."
  - "With annotation `let ages: [int] = []`, the repro compiles clean AND runs with exit 0 via both `ori run` and `ori build` — verified by dual-exec-verify."
  - "No regression in `test_let_polymorphism_for_lambda` — the `let id = x -> x` case type-checks clean after validator integration."
  - "`timeout 150 ./test-all.sh` passes (debug build) and `timeout 150 cargo test --release -p ori_types` passes (release build)."
  - "Known spec-test failures between 03.5 landing and 06.2 audit landing are documented in the Known Failing Tests subsection; no new regressions beyond that documented set."
inspired_by:
  - "Ori `compiler/ori_types/src/check/bodies/mod.rs` — 4-pass bodies architecture per CK-1 (check_function_bodies / check_test_bodies / check_impl_bodies / check_def_impl_bodies) is the direct integration surface."
  - "Rust `rustc_hir_typeck::check_body` — post-body validation pattern: Rust performs wfcheck (well-formed check) AFTER body inference completes, surfacing errors before the IR is used downstream. The same discipline applies here: validate after inference, not during."
  - "Swift `Sema` request-based post-body checks — type-checking requests emit diagnostics as a post-pass step once per body, avoiding cascade from partial inference state."
depends_on: ["01", "02"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "03.1"
    title: "Wire validator into check_function"
    status: not-started
  - id: "03.2"
    title: "Wire validator into check_test"
    status: not-started
  - id: "03.3"
    title: "Wire validator into check_impl_method (TPR checkpoint)"
    status: not-started
  - id: "03.4"
    title: "Wire validator into check_def_impl_method"
    status: not-started
  - id: "03.5"
    title: "End-to-end regression suite and dual-execution parity"
    status: not-started
  - id: "03.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "03.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 03: Bodies-Pass Integration

**Status:** Not Started
**Goal:** Wire `validate_body_types()` (created in Section 02) into all 4 bodies-pass
call sites so that every function, test, impl method, and def-impl method body
surfaces unresolved `Tag::Var`s as E2005 diagnostics before handing typed IR to the
next phase — enforcing the typeck.md PC-2 output contract at the producer boundary.

**Success Criteria:**

- [ ] `grep -c 'validate_body_types' compiler/ori_types/src/check/bodies/mod.rs` returns exactly **4**
- [ ] Rust integration test asserts BUG-04-074 repro emits `TypeErrorKind::AmbiguousType` (E2005)
      AND that no codegen error fires for that input — the error is caught before leaving typeck
- [ ] Annotated repro (`let ages: [int] = []`) compiles clean AND produces exit 0 via both
      interpreter (`ori run`) and AOT (`ori build`) with parity verified by `diagnostics/dual-exec-verify.sh`
- [ ] `test_let_polymorphism_for_lambda` continues to pass (no regression from Section 01)
- [ ] Known-failing spec tests between this section and 06.2 are documented; no undocumented new failures

**Context:** BUG-04-074's root failure mode was exactly the typeck.md PC-2 contract gap:
`check_function` and its three sibling call sites produced typed IR containing unbound
`Tag::Var` (empty-list element types) without emitting any diagnostic. The IR propagated
through canonicalization, ARC lowering, AIMS, and into LLVM codegen where it surfaced as
an "unresolved type variable at codegen" error in `type_info/store.rs:341-363` — a
consumer-side symptom of a producer-side omission. Sections 01 and 02 respectively (a)
stopped the premature generalization of empty-list element vars, and (b) built the
`validate_body_types()` function that can detect and report any surviving `Tag::Var`.
This section closes the loop: it threads the call into every body-checking exit path so
the validator actually runs on every function body. Per `impl-hygiene.md §Narrow the
Front`, all 4 sites are wired together — partial integration (some sites calling the
validator, others not) leaves a gap where E2005 doesn't fire in impl methods or def-impl
methods, which are equally capable of producing unresolved vars.

The integration is mechanically simple — the same 4-line call pattern at each site — but
the placement is load-bearing: the call must happen AFTER `engine.take_expr_types()` has
already extracted the populated map from the engine but BEFORE `checker.push_error()`
drains that data into the checker's error accumulator. Reading the source reveals that
each call site already extracts the 6-tuple `(expr_types, errors, warnings, ...)` from
the inference closure, then iterates over `errors` to push them. The validator call slots
between the extraction and the push — or more precisely it operates on the extracted
`expr_types` and its own errors are pushed alongside the engine's existing errors.

Round 5 research verified:
- `c.pool()` is `ModuleChecker::pool(&self) -> &Pool` at `accessors.rs:72-74`
- `c.arena()` is `ModuleChecker::arena(&self) -> &'a ExprArena` at `accessors.rs:18-21`
- `c.push_error(TypeCheckError)` is the canonical accumulator path
- `engine.take_expr_types()` returns `FxHashMap<ExprIndex, Idx>` (confirmed at `infer/mod.rs`)
- `ExprIndex` is a `usize` type alias (not a newtype), so `span_of` must map usize → Span

**Reference implementations:**

- **Ori** `compiler/ori_types/src/check/bodies/mod.rs` (lines 39-167 for `check_function`,
  175-253 for `check_test`, 319-440 for `check_impl_method`, 462-539 for `check_def_impl_method`)
  — the exact 4 call sites being modified. Each follows the same pattern:
  run inference inside a scope closure, extract `(expr_types, errors, warnings, ...)` as a
  tuple, then iterate the tuple's members to push results onto `checker`.

- **Rust** `compiler/rustc_hir_typeck/src/fn_ctxt/checks.rs` — `check_fn()` performs a
  `wfcheck::check_fn_or_closure()` call AFTER body inference concludes, inside the same
  `FnCtxt` scope that holds the inferred types. The diagnostic is accumulated into the
  shared diagnostic engine (analogous to `checker.push_error()`), not returned.

- **Swift** `lib/Sema/TypeCheckDecl.cpp` — `checkFunctionBody()` triggers a series of
  post-body checkers (definite initialization, effect checking, `@discardableResult` warns)
  after `typeCheckFunctionBodyAtOffset`. Each checker runs against the already-typed body,
  emitting into the engine's diagnostic accumulator.

**Depends on:**
- **Section 01** — provides the Value Restriction policy (`should_generalize`) so that
  lambda-typed bindings are still generalized (they do not produce `Tag::Var`s that the
  validator would spuriously flag) while empty-list element vars remain as Unbound `Tag::Var`
  after inference.
- **Section 02** — provides `ori_types::check::validators::validate_body_types()`
  with the shipped **six-parameter** signature (per `§02.1` + `§02.R` TPR-02-R3-003):
  ```rust
  pub fn validate_body_types(
      pool: &Pool,
      expr_types: &FxHashMap<ExprIndex, Idx>,
      sig: &FunctionSig,
      sig_span: Span,
      span_of: &dyn Fn(ExprIndex) -> Span,
      errors: &mut Vec<TypeCheckError>,
  )
  ```
  The added `sig: &FunctionSig` + `sig_span: Span` parameters extend validator
  scope to the signature (`param_types` + `return_type`) per `typeck.md §CK-4`
  hand-off contract — unannotated params/returns ship from the Signatures pass
  as fresh `Tag::Var` and the validator catches any that survive body inference.
  Module wiring (per `§02.3` + `§02.R` TPR-02-R3-003): `check/mod.rs` declares
  `pub(crate) mod validators;` (private to the crate, not `pub mod`); `lib.rs`
  carries a **narrow re-export** `pub use check::validators::validate_body_types;`
  without promoting `mod check` to `pub mod check` (keeping the entire internal
  check-module layout out of the crate's public API).

**What this section uses from each dependency:**

- From **Section 01**: the guarantee that `should_generalize` gates all 3
  `engine.generalize()` calls, so the only surviving `Tag::Var`s entering the validator are
  genuinely ambiguous (unannotated non-lambda initializers, including empty-list elements).
  Without Section 01, the validator would also flag legitimate lambda polymorphism
  (`let id = x -> x` produces a `Tag::Scheme` in the `expr_types` map, but the body Var
  for the param is resolved at the call site; without Value Restriction the element Var
  would be in a Scheme instead, suppressing the flag).

- From **Section 02**: the `validate_body_types` function itself, imported via
  `use crate::check::validators::validate_body_types;` at the top of `check/bodies/mod.rs`.
  Also Section 02's `span_of` closure contract: the function expects a `&dyn Fn(ExprIndex) -> Span`.
  Each call site must supply a span-lookup closure; the arena provides the span via
  `arena.get_expr(ExprId).span` but the mapping from `ExprIndex` (a `usize`) to `ExprId`
  requires the `expr_types` map to be consulted together with the arena.

---

## 03.1 Wire validator into `check_function`

**File:** `compiler/ori_types/src/check/bodies/mod.rs`
**Function:** `check_function` (line 46–167)
**Insertion point:** After the `with_function_scope` closure completes and the 6-tuple
is destructured (line 148), immediately before the `for (expr_index, ty) in expr_types` loop
that stores expression types into the checker (line 151). The validator runs on the local
`expr_types` variable from the extraction before the types are forwarded into the checker's
module-wide map.

**TDD — write failing test FIRST:**

Before changing production code, add an integration test in
`compiler/ori_types/src/check/bodies/tests.rs` (create the sibling if it doesn't exist —
the existing `#[cfg(test)] mod tests;` declaration at line 542 of `bodies/mod.rs` already
covers this):

```rust
/// Regression: unannotated empty list binding with no element-constraining use
/// should produce E2005 (AmbiguousType) at typeck, not at codegen.
///
/// See: plans/empty-container-typeck-phase-contract/section-03-bodies-pass-integration.md
#[test]
fn check_function_with_unannotated_empty_list_emits_ambiguous_type() {
    // Build: @main () -> int = { let ages = []; if ages.len() == 1 then 0 else 1 }
    // Expected: TypeCheckError with code E2005 (AmbiguousType) for the `ages` binding.
    // The test exercises check_function (Pass 2 per CK-1).
    //
    // Before 03.1 implementation: test fails (no E2005, or codegen-path error instead).
    // After 03.1: test passes.
    let result = compile_snippet_for_errors("@main () -> int = { let ages = []; if ages.len() == 1 then 0 else 1 }");
    assert!(
        result.errors.iter().any(|e| matches!(e.kind(), TypeErrorKind::AmbiguousType { .. })),
        "expected E2005 AmbiguousType, got: {:?}",
        result.errors
    );
}
```

The test helper `compile_snippet_for_errors` is assumed to mirror whatever the existing
`bodies/tests.rs` sibling uses for single-snippet type-checking. Verify the helper pattern
by reading the existing sibling test file before writing.

**Implementation — before/after diff:**

The call pattern inserts between the 6-tuple extraction (line 148 closing `});`) and the
`for (expr_index, ty) in expr_types` storage loop (line 151).

**Before** (abridged, lines 148-157):

```rust
        });  // closes with_function_scope

    // Store expression types
    for (expr_index, ty) in expr_types {
        checker.store_expr_type(expr_index, ty);
    }

    // Store errors and warnings
    for error in errors {
        checker.push_error(error);
    }
```

**After** (the insertion point is the blank line before "Store expression types"):

```rust
        });  // closes with_function_scope

    // Validate PC-2 contract: no unbound Tag::Var in body expr_types
    // OR in the body's FunctionSig (param_types + return_type).
    // Runs after inference (expr_types fully populated) and before the types
    // are forwarded to the module-checker's map, so errors flow through the
    // normal push_error accumulator alongside inference errors.
    // Spec: docs/ori_lang/v2026/spec/14-expressions.md:1224-1228
    // Plan: plans/empty-container-typeck-phase-contract/section-03-bodies-pass-integration.md §03.1
    {
        let arena = checker.arena();
        let pool = checker.pool();
        let sig_span = arena.get_expr(fn_decl.expr_id).span; // function decl span
        let mut validation_errors = Vec::new();
        crate::check::validators::validate_body_types(
            pool,
            &expr_types,
            sig,             // &FunctionSig from signatures pass
            sig_span,        // function declaration span for sig-origin diagnostics
            &|expr_index| {
                // ExprIndex is a usize alias for ExprId.raw() — the InferEngine
                // records (ExprIndex, Idx) pairs keyed by the ExprId's raw u32
                // value (verified at compiler/ori_types/src/infer/mod.rs:56).
                // Convert back: ExprId::from_raw(expr_index as u32).
                arena.get_expr(ori_ir::ExprId::from_raw(expr_index as u32)).span
            },
            &mut validation_errors,
        );
        for err in validation_errors {
            checker.push_error(err);
        }
    }

    // Store expression types
    for (expr_index, ty) in expr_types {
        checker.store_expr_type(expr_index, ty);
    }

    // Store errors and warnings
    for error in errors {
        checker.push_error(error);
    }
```

**Span lookup note:** The `span_of` closure is a placeholder in the diff above. At
implementation time, read `compiler/ori_types/src/infer/mod.rs` to find the function
that records `(ExprIndex, Idx)` pairs into `expr_types`. The `ExprIndex` value recorded
there must map back to a span. If `ExprIndex` == `ExprId.raw()` (which is the pattern
used at `infer/mod.rs:56` per Section 02's research), then the span lookup is:

```rust
&|expr_index| {
    // ExprIndex is ExprId.raw() — reconstruct the ExprId and look up its span.
    let expr_id = ori_ir::ExprId::from_raw(expr_index as u32);
    arena.get_expr(expr_id).span
},
```

Verify that `ExprId::from_raw` exists (check `ori_ir/src/ast/expr/id.rs` or similar)
and that the relationship `ExprIndex == ExprId.raw()` holds before using this pattern.
If the relationship is different, construct the appropriate lookup.

- [ ] **TDD first** — add `check_function_with_unannotated_empty_list_emits_ambiguous_type`
  to `bodies/tests.rs`. Run `timeout 150 cargo test -p ori_types -- bodies::tests` and
  confirm it FAILS before the code change (the test is the regression pin).
- [ ] Verify the exact span-lookup API for `ExprIndex → Span` by reading `infer/mod.rs`
  and `ori_ir/src/ast/expr/`.
- [ ] Add `use crate::check::validators;` import at the top of `bodies/mod.rs` (after the
  existing `use super::...` imports, in the crate-relative import group).
- [ ] Insert the validator call block at line 148 of `bodies/mod.rs` (after the closing
  `})` of `with_function_scope`, before "Store expression types").
- [ ] Run `timeout 150 cargo test -p ori_types -- bodies::tests::check_function_with_unannotated_empty_list_emits_ambiguous_type`
  and confirm it **PASSES**.
- [ ] Run `timeout 150 cargo test -p ori_types` (all tests) — no regressions.
- [ ] **Subsection close-out (03.1)** — MANDATORY before starting 03.2:
  - [ ] All tasks above are `[x]` and behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — reflect on the
        debugging journey for 03.1 specifically. Was the span-lookup API immediately
        obvious or did it require archaeology? Was the insertion point easy to find?
        Would a diagnostic flag (e.g., `ORI_LOG=ori_types=debug` showing "validator
        skipped — expr_types empty") have saved time? Implement every accepted improvement
        NOW (zero deferral) and commit via SEPARATE `/commit-push`. Use a valid
        conventional-commit type (`build`, `test`, `chore`, `ci`, `docs` — NOT `tools`).
        If no gaps, document: "Retrospective 03.1: no tooling gaps."
  - [ ] **Run `/sync-claude` on THIS subsection** — did adding a `use crate::check::validators`
        import or the validator call change any public API, command, or pipeline phase
        behavior documented in CLAUDE.md / rules? If all three questions are "no," document:
        "Claude artifact sync 03.1: no API/command/phase changes — artifacts current."
  - [ ] **Repo hygiene** — run `diagnostics/repo-hygiene.sh --check` and clean any temp files.

---

## 03.2 Wire validator into `check_test`

**File:** `compiler/ori_types/src/check/bodies/mod.rs`
**Function:** `check_test` (line 182–253)
**Insertion point:** After the manual 6-tuple extraction (lines 229-235) and before the
`for (expr_index, ty) in expr_types` storage loop (line 237). Note that `check_test` does
NOT use `with_function_scope` — it creates the engine directly and calls `take_*` methods
on it. The insertion sits between the last `engine.take_*()` call and the first storage loop.

**Current structure (lines 229-252, abridged):**

```rust
    // Extract results
    let expr_types = engine.take_expr_types();
    let errors = engine.take_errors();
    let warnings = engine.take_warnings();
    let pat_resolutions = engine.take_pattern_resolutions();
    let mono_instances = engine.take_mono_instances();
    let deferred_mono_calls = engine.take_deferred_mono_calls();

    // Store expression types
    for (expr_index, ty) in expr_types {
        checker.store_expr_type(expr_index, ty);
    }
```

**After insertion:**

```rust
    // Extract results
    let expr_types = engine.take_expr_types();
    let errors = engine.take_errors();
    // ... (remaining take_* calls unchanged) ...

    // Validate PC-2 contract: no unbound Tag::Var in body expr_types.
    // Spec: docs/ori_lang/v2026/spec/14-expressions.md:1224-1228
    // Plan: plans/empty-container-typeck-phase-contract/section-03-bodies-pass-integration.md §03.2
    {
        let pool = checker.pool();
        let arena = checker.arena();
        let mut validation_errors = Vec::new();
        crate::check::validators::validate_body_types(
            pool,
            &expr_types,
            &|expr_index| {
                let expr_id = ori_ir::ExprId::from_raw(expr_index as u32);
                arena.get_expr(expr_id).span
            },
            &mut validation_errors,
        );
        for err in validation_errors {
            checker.push_error(err);
        }
    }

    // Store expression types
    for (expr_index, ty) in expr_types { ... }
```

The call pattern is identical to 03.1; only the host function and the insertion line
number differ. In `check_test`, `arena` was already bound earlier in the function body
(line 200: `let arena = checker.arena();`) so the `let arena =` inside the scope block
is a re-borrow of the checker — verify this compiles cleanly given lifetime constraints
(`arena()` returns `&'a ExprArena` tied to the checker's `'a` lifetime, not to `&self`).
If there is a borrow conflict with `checker.pool()`, use the split-borrow pattern:
bind `let arena = checker.arena();` before calling `checker.pool()`, or separate the
borrows temporally.

**TDD — failing test:**

```rust
/// Test-body with unannotated empty list produces E2005 at typeck, not at codegen.
/// Exercises check_test (Pass 3 per CK-1).
///
/// See: plans/empty-container-typeck-phase-contract/section-03-bodies-pass-integration.md §03.2
#[test]
fn check_test_with_unannotated_empty_list_emits_ambiguous_type() {
    let result = compile_test_body_for_errors(
        "@t tests @fn () -> void = { let xs = []; assert_eq(xs.len(), 0) }",
    );
    assert!(
        result.errors.iter().any(|e| matches!(e.kind(), TypeErrorKind::AmbiguousType { .. })),
        "expected E2005 AmbiguousType in test body, got: {:?}",
        result.errors
    );
}
```

- [ ] **TDD first** — add the test to `bodies/tests.rs`, confirm it FAILS before the code change.
- [ ] Check for borrow-lifetime conflicts in `check_test` (the `arena` variable may already be
  in scope — see line 200; either re-use it or confirm the split-borrow compiles cleanly).
- [ ] Insert the validator call block after the last `engine.take_*()` extraction, before the
  storage loop.
- [ ] Confirm the new test **PASSES**.
- [ ] `timeout 150 cargo test -p ori_types` — no regressions.
- [ ] **Subsection close-out (03.2)** — MANDATORY before starting 03.3:
  - [ ] All tasks above are `[x]` and behavior is verified
  - [ ] Update subsection `status` in frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — commit improvements
        separately using a valid conventional-commit type.
  - [ ] **Repo hygiene** — `diagnostics/repo-hygiene.sh --check`.

---

## 03.3 Wire validator into `check_impl_method` (TPR checkpoint)

**File:** `compiler/ori_types/src/check/bodies/mod.rs`
**Function:** `check_impl_method` (line 320–440)
**Insertion point:** After the nested closure that calls `with_impl_scope` → `with_function_scope`
returns its 6-tuple (line 390, closing `});`), before the
`for (expr_index, ty) in expr_types` storage loop (line 393).

The nested scope structure in `check_impl_method` mirrors `check_function`:

```rust
    let (expr_types, errors, warnings, pat_resolutions, mono_instances, deferred_mono_calls) =
        checker.with_impl_scope(self_type, |c| {
            c.with_function_scope(fn_type, FxHashSet::default(), |c| {
                // ... inference ...
                (
                    engine.take_expr_types(),
                    engine.take_errors(),
                    ...
                )
            })
        });

    // Store results   ← INSERT VALIDATOR CALL HERE, between these two blocks
    for (expr_index, ty) in expr_types {
```

The validator call pattern is identical to 03.1/03.2. The `arena` binding: inside
`check_impl_method` there is no pre-existing `arena` binding in scope after the closure;
`checker.arena()` is fresh. The `pool` and `arena` borrows from `checker` are read-only
and do not conflict with `checker.push_error` (which takes `&mut self`) — use a scoped
block to ensure the borrows end before `push_error`.

**TDD — failing test:**

```rust
/// Impl method body with unannotated empty list produces E2005 at typeck.
/// Exercises check_impl_method (Pass 4 per CK-1).
///
/// See: plans/empty-container-typeck-phase-contract/section-03-bodies-pass-integration.md §03.3
#[test]
fn check_impl_method_with_unannotated_empty_list_emits_ambiguous_type() {
    let result = compile_impl_method_for_errors(
        "type Foo = {}  impl Foo { @items (self) -> [int] = { let xs = []; xs } }",
    );
    assert!(
        result.errors.iter().any(|e| matches!(e.kind(), TypeErrorKind::AmbiguousType { .. })),
        "expected E2005 AmbiguousType in impl method body, got: {:?}",
        result.errors
    );
}
```

- [ ] **TDD first** — add the test to `bodies/tests.rs`, confirm it FAILS.
- [ ] Insert the validator call block after the closing `});` of `with_impl_scope`
  (line 390), before "Store results".
- [ ] Confirm the new test **PASSES**.
- [ ] `timeout 150 cargo test -p ori_types` — no regressions.
- [ ] **Subsection close-out (03.3)** — MANDATORY before starting 03.4:
  - [ ] All tasks above are `[x]` and behavior is verified
  - [ ] Update subsection `status` in frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Repo hygiene** — `diagnostics/repo-hygiene.sh --check`

- [ ] **TPR checkpoint** — run `/tpr-review` covering subsections 03.1, 03.2, and 03.3
  before proceeding to 03.4. The three integration sites together expose the call-pattern
  SSOT (all three sites should be structurally identical — any divergence is a finding).
  Resolve all critical and major findings before proceeding. Record findings in 03.R.

---

## 03.4 Wire validator into `check_def_impl_method`

**File:** `compiler/ori_types/src/check/bodies/mod.rs`
**Function:** `check_def_impl_method` (line 462–539)
**Insertion point:** After the `with_function_scope` closure returns its 6-tuple (line 525,
closing `});`), before the `for (expr_index, ty) in expr_types` storage loop (line 528).

`check_def_impl_method` uses only `with_function_scope` (no `with_impl_scope` — def-impl
methods are stateless, per the comment at line 492). The structure is:

```rust
    let (expr_types, errors, warnings, ...) =
        checker.with_function_scope(fn_type, FxHashSet::default(), |c| {
            // ... inference ...
            (engine.take_expr_types(), engine.take_errors(), ...)
        });

    // Store results   ← INSERT VALIDATOR CALL HERE
    for (expr_index, ty) in expr_types {
```

The call pattern is identical to 03.1. No structural differences from `check_function`.

**TDD — failing test:**

```rust
/// Def-impl method body with unannotated empty list produces E2005 at typeck.
/// Exercises check_def_impl_method (Pass 5 per CK-1).
///
/// See: plans/empty-container-typeck-phase-contract/section-03-bodies-pass-integration.md §03.4
#[test]
fn check_def_impl_method_with_unannotated_empty_list_emits_ambiguous_type() {
    let result = compile_def_impl_method_for_errors(
        "trait Fooable { @items () -> [int] }  pub def impl Fooable { @items () -> [int] = { let xs = []; xs } }",
    );
    assert!(
        result.errors.iter().any(|e| matches!(e.kind(), TypeErrorKind::AmbiguousType { .. })),
        "expected E2005 AmbiguousType in def-impl method body, got: {:?}",
        result.errors
    );
}
```

- [ ] **TDD first** — add the test, confirm it FAILS.
- [ ] Insert the validator call block after the closing `});` of `with_function_scope`
  (line 525), before "Store results".
- [ ] Confirm the new test **PASSES**.
- [ ] Verify the grep criterion: `grep -c 'validate_body_types' compiler/ori_types/src/check/bodies/mod.rs`
  returns **4** (all four sites now call the validator).
- [ ] `timeout 150 cargo test -p ori_types` — no regressions.
- [ ] **Subsection close-out (03.4)** — MANDATORY before starting 03.5:
  - [ ] All tasks above are `[x]` and behavior is verified
  - [ ] Update subsection `status` in frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — now that all 4 sites
        are implemented, check for algorithmic DRY violations: the 4-line call block is
        identical at all 4 sites. Consider extracting a `run_validator(checker, &expr_types) -> Vec<TypeCheckError>` helper to eliminate the duplication (per `impl-hygiene.md §Algorithmic
        DRY` — same control-flow skeleton at 4 sites = missing abstraction). If the helper
        reduces code while remaining readable, implement it and commit separately.
  - [ ] **Run `/sync-claude` on THIS subsection** — 03.4 completes all 4 sites; the
        `check/bodies/mod.rs` file now imports `crate::check::validators`. Does this change
        any rules file claim? If `typeck.md` mentions "the bodies pass does not validate
        PC-2 inline," update it.
  - [ ] **Repo hygiene** — `diagnostics/repo-hygiene.sh --check`

---

## 03.5 End-to-end regression suite and dual-execution parity

**Files touched:**
- `compiler/ori_llvm/tests/aot/` — new AOT integration test
- `compiler/ori_types/src/check/bodies/tests.rs` — BUG-04-074 repro assertion

This subsection verifies the full round-trip: the original BUG-04-074 repro (unannotated)
produces E2005 at typeck; the annotated version compiles and runs correctly via both the
interpreter and AOT path.

### 03.5.1 — BUG-04-074 repro: unannotated form rejects at typeck

**Repro program:**

```ori
@main () -> int = {
    let ages = [];
    ages = ages.push(value: 10);
    if ages.len() == 1 then 0 else 1
}
```

This is the exact program from BUG-04-074. After Sections 01–03.4, this program MUST:
1. Emit `E2005: cannot infer type for empty list — add a type annotation like 'let ages: [int] = []'`
2. NOT proceed to codegen (per typeck.md PC-4 — codegen is gated on zero typeck errors)
3. NOT produce an "unresolved type variable at codegen" error

Add an integration test in `compiler/ori_types/src/check/bodies/tests.rs`:

```rust
/// Full BUG-04-074 repro: unannotated empty list with push + len usage.
/// E2005 must fire at typeck; no codegen-path error may fire.
///
/// See: plans/empty-container-typeck-phase-contract/00-overview.md §Mission Success Criteria
#[test]
fn empty_list_with_push_and_len_rejects_at_typeck_with_ambiguous_type() {
    let result = compile_program_for_errors(
        "@main () -> int = { let ages = []; ages = ages.push(value: 10); if ages.len() == 1 then 0 else 1 }",
    );
    // At least one E2005 for the empty list.
    let has_e2005 = result.errors.iter().any(|e| {
        matches!(e.kind(), TypeErrorKind::AmbiguousType { .. })
    });
    assert!(has_e2005, "expected E2005 for unannotated empty list, errors: {:?}", result.errors);
    // No codegen-path errors — the error must be caught before reaching codegen.
    let has_codegen_error = result.errors.iter().any(|e| {
        // Codegen errors are in a different crate; what we verify here is that
        // ori_types emitted E2005, which gates codegen via PC-4. If the test
        // suite later adds an end-to-end codegen path, assert no LLVM errors.
        e.is_internal_compiler_error()
    });
    assert!(!has_codegen_error, "codegen-path error fired despite E2005 gate");
}
```

### 03.5.2 — Annotated form compiles and runs: AOT integration test

**Annotated repro program:**

```ori
@main () -> int = {
    let ages: [int] = [];
    ages = ages.push(value: 10);
    if ages.len() == 1 then 0 else 1
}
```

This program MUST compile clean AND exit with code 0.

Add an AOT test in `compiler/ori_llvm/tests/aot/`:

```rust
/// Annotated empty list with push + len compiles clean and exits 0.
/// Confirms the repro from BUG-04-074 works correctly with a type annotation.
///
/// Dual-execution: both interpreter and AOT paths must produce exit 0.
#[test]
fn annotated_empty_list_with_push_and_len_compiles_and_exits_zero() {
    let src = r#"
        @main () -> int = {
            let ages: [int] = [];
            ages = ages.push(value: 10);
            if ages.len() == 1 then 0 else 1
        }
    "#;
    // AOT path
    let exit_code = run_aot_program(src);
    assert_eq!(exit_code, 0, "annotated empty list should exit 0 via AOT");

    // Interpreter path (dual-execution parity)
    let exit_code = run_interpreter_program(src);
    assert_eq!(exit_code, 0, "annotated empty list should exit 0 via interpreter");
}
```

If `run_aot_program` / `run_interpreter_program` helpers don't exist in the AOT test
harness, check the existing test files in `compiler/ori_llvm/tests/aot/` for the
correct pattern and use the matching helpers.

### 03.5.3 — Dual-execution parity via diagnostic script

Run `diagnostics/dual-exec-verify.sh` against the annotated repro program to confirm
the interpreter and LLVM produce identical observable results. This is the plan-level
dual-execution gate referenced in `00-overview.md §Mission Success Criteria`.

```bash
# Create a temporary test file matching the annotated repro
echo '@main () -> int = {
    let ages: [int] = [];
    ages = ages.push(value: 10);
    if ages.len() == 1 then 0 else 1
}' > /tmp/ages_repro.ori
diagnostics/dual-exec-verify.sh /tmp/ages_repro.ori
# Expected: "PASS: interpreter and AOT produce identical output (exit 0)"
```

### 03.5.4 — Full test suite

Run `timeout 150 cargo st` (spec tests). Document any new failures in the Known Failing
Tests section below. Do NOT investigate them individually — Section 06.2 resolves them
via annotation. Confirm that the number of newly-failing spec tests is bounded and
explainable (they are programs with `[]` or empty collection literals lacking type context).

- [ ] Add integration test `empty_list_with_push_and_len_rejects_at_typeck_with_ambiguous_type`
  to `check/bodies/tests.rs` — confirms the unannotated repro produces E2005 at typeck.
- [ ] Add AOT test `annotated_empty_list_with_push_and_len_compiles_and_exits_zero`
  in `compiler/ori_llvm/tests/aot/`.
- [ ] Run `diagnostics/dual-exec-verify.sh` against the annotated repro — confirm parity.
- [ ] Run `timeout 150 cargo st` — document newly-failing spec tests in the section below.
- [ ] Run `timeout 150 ./test-all.sh` — ensure no regressions beyond the documented set.
- [ ] Run `timeout 150 cargo test --release -p ori_types` (release-build parity).
- [ ] **Subsection close-out (03.5)** — MANDATORY before starting 03.R:
  - [ ] All tasks above are `[x]` and behavior is verified
  - [ ] Update subsection `status` in frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — was the
        dual-exec-verify script immediately useful? Did the AOT test setup require
        boilerplate archaeology? Implement improvements and commit separately.
  - [ ] **Run `/sync-claude` on THIS subsection** — the annotated-form test touches
        `ori_llvm/tests/aot/`. Does any `compiler.md` or `typeck.md` claim need updating
        (e.g., "AOT tests for empty-container correctness exist at path X")?
  - [ ] **Repo hygiene** — `diagnostics/repo-hygiene.sh --check`

---

## Known Failing Tests (Expected Until Section 06.2 Lands)

Once the validator fires on all 4 bodies-pass sites, **any spec-test program that uses
an empty collection literal without a type annotation will now correctly emit E2005**.
These are spec-test corpus programs that were PREVIOUSLY compiled by accident (the
compiler failed to reject them per spec). After Section 03.5 lands, they will fail until
Section 06.2 audits the corpus and adds annotations.

The following categories of programs are expected to fail between 03.5 and 06.2:

| Category | Example pattern | Section 06.2 resolution |
|----------|----------------|-------------------------|
| `let x = []` with no annotation and no type-constraining use | `tests/spec/` programs using empty-list literals as sentinel values | Add `let x: [T] = []` annotation |
| `[].iter()` chained calls where the element type is never constrained | `tests/spec/traits/iterator/` patterns like `[].iter().count()` | Add explicit type or move to typed literal |
| `[].is_empty()` / `[].len()` without a type constraint | Various collections tests | Annotate or use non-empty alternatives |
| `{}.keys()` or similar empty-map patterns | `tests/spec/collections/` | Annotate `let m: {str: int} = {}` |

**To populate this list accurately:** when running `timeout 150 cargo st` in 03.5.4,
capture the output and filter for `E2005`. Each failing spec-test file is a row in the
table above's "Annotate or use non-empty alternatives" resolution category. Commit the
updated table alongside the 03.5 implementation. Do NOT fix individual failing tests —
that is Section 06.2's job.

**Per `00-overview.md §Known Bugs`:**
> `TPR-04-005-codex` audit finding: `tests/spec/` uses `[].iter()`, `[].is_empty()`
> patterns beyond just `let x = []` bindings; these WILL trip E2005 once live.
> Section 06.2 resolves them via annotation.

---

## 03.R Third Party Review Findings

<!-- Reserved for the dual-source `/tpr-review` (Codex + Gemini) and other external reviewers.
If unresolved findings exist here:
- section frontmatter `status` must be `in-progress`
- `third_party_review.status` must be `findings`

When all findings are triaged:
- accepted findings are integrated into the relevant implementation subsection(s)
- rejected findings are closed with rationale
- `third_party_review.status` becomes `resolved` or `none`
-->

- None.

---

## 03.N Completion Checklist

- [ ] `grep -c 'validate_body_types' compiler/ori_types/src/check/bodies/mod.rs` returns **4**
  (all 4 bodies-pass functions call the validator)
- [ ] `grep -n 'use crate::check::validators' compiler/ori_types/src/check/bodies/mod.rs`
  shows exactly one import line
- [ ] All 4 bodies-pass tests pass:
  - [ ] `check_function_with_unannotated_empty_list_emits_ambiguous_type`
  - [ ] `check_test_with_unannotated_empty_list_emits_ambiguous_type`
  - [ ] `check_impl_method_with_unannotated_empty_list_emits_ambiguous_type`
  - [ ] `check_def_impl_method_with_unannotated_empty_list_emits_ambiguous_type`
- [ ] End-to-end regression tests pass:
  - [ ] `empty_list_with_push_and_len_rejects_at_typeck_with_ambiguous_type` (typeck gate)
  - [ ] `annotated_empty_list_with_push_and_len_compiles_and_exits_zero` (AOT + dual-exec)
- [ ] `diagnostics/dual-exec-verify.sh` confirms interpreter/AOT parity on the annotated repro
- [ ] `test_let_polymorphism_for_lambda` still passes (Section 01 regression pin intact)
- [ ] No undocumented spec-test failures — Known Failing Tests section is accurate and complete
- [ ] The call block structure is DRY — either a shared `run_validator` helper exists, or
  the 4-line pattern is consistently identical at all 4 sites with no local deviations
- [ ] All plan-annotation comments (`# Plan: ...`, `§03.N`) use the correct section reference
  and will be stripped by the Section 07 annotation-cleanup pass
- [ ] All intermediate subsection close-out tasks complete (03.1–03.5)
- [ ] **Plan sync** — update plan metadata to reflect section completion:
  - [ ] This section's frontmatter `status` → `complete`, all subsection statuses updated
  - [ ] `00-overview.md` Quick Reference table entry for Section 03 updated to `Complete`
  - [ ] `00-overview.md` mission success criteria: check off the BUG-04-074 repro criterion
        if now satisfied, and the "typeck rejection" criterion if now satisfied
  - [ ] Section 04's `depends_on` references Section 03 — verify Section 04's assumptions
        still hold (specifically: that the producer side is clean so debug_assert!s in
        codegen won't fire on legitimate typed IR)
- [ ] `timeout 150 ./test-all.sh` green (debug build)
- [ ] `timeout 150 cargo test --release -p ori_types` green (release build)
- [ ] `timeout 150 ./clippy-all.sh` clean (no new warnings)
- [ ] `/tpr-review` passed (final, full-section) — independent dual-source review (Codex +
  Gemini) found no critical or major issues, or all findings triaged and recorded in 03.R.
  This is in ADDITION to the intermediate TPR checkpoint at 03.3.
- [ ] `/impl-hygiene-review` passed — MUST run AFTER `/tpr-review` is clean. Auto-scope:
  the active work arc (`git diff` since Section 01 started). Never use `last commit` scope
  — it is too narrow for multi-subsection work. Key areas to review: algorithmic DRY on
  the 4-site call pattern, import hygiene, test function naming (no ephemeral identifiers),
  phase-boundary invariant comment quality.
- [ ] `/improve-tooling` **section-close sweep** — verify every subsection (03.1–03.5) has
  either an "improvements made" entry (with commits) or a documented "no gaps" negative
  finding from its per-subsection retrospective. Look for cross-subsection patterns invisible
  at per-item scope. Add only new items from cross-cutting patterns; implement immediately,
  commit separately. If no new patterns found, document: "Section-close sweep: per-subsection
  retrospectives covered everything; no cross-subsection patterns required new tooling."
- [ ] `/sync-claude` **section-close doc sync** — run across all commits in Section 03
  (`git diff --name-only <section-03-start>..HEAD`). Map changed files to rules (primarily
  `typeck.md §PC-2` for the producer-side enforcement that is now live; `canon.md §4.2`
  for the PC-2 output contract being enforced; `CLAUDE.md §Type Checker Patterns` for
  any new patterns). Verify each rules file is accurate. Fix any drift and commit.
  Document result.
- [ ] **Repo hygiene check** — `diagnostics/repo-hygiene.sh --check` clean before final commit

**Exit Criteria:** All 4 bodies-pass sites call `validate_body_types`. BUG-04-074 repro
emits E2005 at typeck (not at codegen). Annotated form compiles and runs with exit 0 via
interpreter and AOT with parity verified. `test_let_polymorphism_for_lambda` passes.
Known spec-test failures are documented. `timeout 150 ./test-all.sh` green. Both
`/tpr-review` (full-section and intermediate 03.3 checkpoint) and `/impl-hygiene-review`
clean. Section 04 can now enable its `debug_assert!` hooks without them firing on
legitimate well-typed IR.
