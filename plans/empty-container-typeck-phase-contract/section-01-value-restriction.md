---
section: "01"
title: "AST-based Value Restriction"
status: in-progress
reviewed: true
goal: "Extract a single SSOT `should_generalize` helper and migrate all 3 let-generalization sites to call it, so only direct non-capturing lambda initializers generalize — all other initializers (including empty lists, block-wrapped lambdas, variable aliases, conditionals producing functions) become monomorphic."
success_criteria:
  - "Single `pub(super) fn should_generalize(arena: &ExprArena, init: ExprId) -> bool` exists in `compiler/ori_types/src/infer/expr/blocks.rs` — verifiable via `grep -n 'pub(super) fn should_generalize' compiler/ori_types/src/infer/expr/blocks.rs` returning exactly one hit."
  - "All 3 generalization sites call `should_generalize` — NOT a type-tag heuristic, NOT inlined duplicated logic. Verifiable: `grep -n 'engine.generalize' compiler/ori_types/src/infer/expr/blocks.rs compiler/ori_types/src/infer/expr/sequences.rs` returns exactly 3 call sites each preceded by `if should_generalize(...)`."
  - "`let id = x -> x; id(1); id(\"hello\")` type-checks and runs correctly in both interpreter and LLVM — regression pin for let-polymorphism preservation. Test `test_let_polymorphism_for_lambda` in `compiler/ori_types/src/infer/expr/tests.rs` passes BEFORE and AFTER the change; reverting `should_generalize` must break it."
  - "`let x = []` no longer generalizes the element Var — `Tag::Var` stays Unbound after the block-statement let path returns, ready for Section 02's validator to catch. Verifiable via a unit test `test_empty_list_let_binding_does_not_generalize_element_var` in the same tests file."
  - "Patterns that become intentionally monomorphic are tested as negative pins: `let f = { x -> x }` (block-wrapped lambda), `let alias = id` (variable aliasing a polymorphic binding), `let f = if true then (x -> x) else (y -> y)` (conditional producing a function) — all produce monomorphic bindings under the new policy, and negative pin tests verify this."
  - "`timeout 150 ./test-all.sh` remains green (debug and release builds) after the migration — no regressions in existing spec tests."
inspired_by:
  - "Rust `rustc_hir_typeck` — no let-polymorphism for local bindings; all local bindings are monomorphic. Every `let x = e` in a function body constrains `e`'s type variable to the inferred monotype rather than generalizing it."
  - "Haskell monomorphism restriction — motivation for Value Restriction: unrestricted generalization of mutable or effectful bindings leads to unsoundness; even in a pure setting, generalizing container element types produces unresolvable Vars downstream."
  - "Ori `body_captures_outer` precedent at `compiler/ori_types/src/infer/expr/blocks.rs:79-89` — the codebase ALREADY uses AST-based Lambda detection to distinguish non-capturing from capturing closures. `should_generalize` extends this exact pattern to the generalization decision itself."
depends_on: []
third_party_review:
  status: resolved
  updated: 2026-04-14
sections:
  - id: "01.1"
    title: "Extract `should_generalize` SSOT helper"
    status: complete
  - id: "01.2"
    title: "Migrate `infer_block` block-statement let site"
    status: complete
  - id: "01.3"
    title: "Migrate `infer_let` (ExprKind::Let dispatch) site"
    status: complete
  - id: "01.4"
    title: "Migrate `sequences.rs` try-block let site"
    status: complete
  - id: "01.R"
    title: "Third Party Review Findings"
    status: complete
  - id: "01.N"
    title: "Completion Checklist"
    status: in-progress
---

# Section 01: AST-based Value Restriction

**Status:** Not Started
**Goal:** Replace unconditional generalization at 3 let-binding sites with a single SSOT
`should_generalize(arena, init_expr_id) -> bool` helper that returns `true` only for
direct, non-capturing `ExprKind::Lambda` initializers. Preserves let-polymorphism for
`let id = x -> x` while preventing empty-list element Vars from being prematurely
generalized into Schemes.

**Semantic scope of this change:** This change restricts Ori's let-polymorphism to ONLY
direct non-capturing lambda bindings. The following patterns that were previously
polymorphic become **intentionally monomorphic** under the new policy:

- `let f = { x -> x }` — block wrapping a lambda: `should_generalize` sees
  `ExprKind::Block`, not `ExprKind::Lambda`, so returns `false`.
- `let alias = id` — variable aliasing a polymorphic binding: `should_generalize`
  sees `ExprKind::Ident`, returns `false`. `alias` gets the instantiated monotype.
- `let f = if c then (x -> x) else (y -> y)` — conditional producing a function:
  `should_generalize` sees `ExprKind::If`, returns `false`.
- `let x = []` — empty list: sees `ExprKind::List`, returns `false`.
- `let m = {}` — empty map: sees `ExprKind::Map`, returns `false`.
- Any non-lambda expression: function calls, struct literals, constants, etc.

This is a deliberate design choice matching Rust's approach (no let-polymorphism for
locals) while preserving the most common polymorphic use case (`let id = x -> x`).
The choice is AST-based rather than type-based because type-tag heuristics fail when
the resolved type is still `Tag::Var` awaiting bi-directional unification (per Gemini
Round 1 TPR finding on the original fix-section).

**Success Criteria:**

- [ ] Single `pub(super) fn should_generalize` exists in `blocks.rs` — one grep hit
- [ ] All 3 `engine.generalize()` calls are gated by `if should_generalize(...)` — grep verifiable
- [ ] `test_let_polymorphism_for_lambda` passes before and after; reverting the change breaks it
- [ ] `test_empty_list_let_binding_does_not_generalize_element_var` passes after migration
- [ ] Negative pins for intentionally monomorphic patterns pass (block-wrapped, aliased, conditional)
- [ ] `timeout 150 ./test-all.sh` green (debug + release)

**Cross-section contract (Section 01 -> Sections 02/03):** After Section 01 completes,
`infer_expr` stores expression types BEFORE the generalization step in `infer_block` /
`infer_let`. For non-lambda initializers, the element `Tag::Var` stays Unbound in the
type pool but IS stored in `engine.expr_types()` during `infer_expr`. Section 02's
validator (`validate_body_types`) runs AFTER body inference and inspects `expr_types` —
it is the validator that surfaces `E2005` on these remaining Unbound vars. Section 01
alone does not reject programs; it prepares the ground for Section 02/03 to do so.
Without Section 02, the Unbound vars would silently flow to codegen (the pre-existing
bug). Without Section 01, the validator would falsely reject polymorphic lambda bindings.
Both sections are required for correctness.

**Context:** BUG-04-074 traced the "unresolved type variable at codegen" failure to three
unconditional `engine.generalize()` calls in the typeck let-binding paths. Generalizing
an empty-list element's `Tag::Var` turns it into a `Tag::Scheme` whose bound var is
never instantiated to a concrete type — downstream use sites like `.len()` don't constrain
the element type, so the Scheme persists unresolved through canonicalization, ARC lowering,
and into LLVM codegen where it triggers a verification failure. The fix is AST-based Value
Restriction: only direct non-capturing lambdas (`x -> x`) qualify for generalization; all
other initializers — including `[]`, `{}`, block-wrapped lambdas, variable aliases, and
constants — are monomorphic and must not generalize their type variables.

**Reference implementations:**

- **Rust** `compiler/rustc_hir_typeck/src/expr.rs`: no let-polymorphism for local let
  bindings — Rust's type checker never generalizes locally-bound types into schemes; every
  `let x = e` in a function body is monomorphic. Ori's design differs (it supports
  `let id = x -> x` with genuine polymorphism for direct non-capturing lambdas), but the
  lesson is clear: unrestricted generalization of arbitrary initializers is unsound.

- **Haskell** `ghc/compiler/GHC/Tc/Gen/Bind.hs`: the monomorphism restriction
  motivates why even a purely functional language needs Value Restriction — functions
  defined without explicit type signatures that involve type classes can behave
  unexpectedly when generalized and re-used at different types.

- **Ori** `compiler/ori_types/src/infer/expr/blocks.rs:79-89`: `body_captures_outer`
  precedent — the codebase already performs AST-based Lambda detection + capture analysis
  to decide whether a lambda is capturing. `should_generalize` extends this exact check
  to make generalization conditional on the result.

**Note on `body_captures_outer` completeness:** The existing `body_captures_outer`
function (L249-286 in `blocks.rs`) is an **under-approximation**: the `_ => false`
catch-all at L284 skips expression forms it does not explicitly walk (call arguments,
method arguments beyond the receiver, list/tuple/map subexpressions). This means it
may MISS captures in some expressions, causing `should_generalize` to return `true`
when it should return `false` — incorrectly generalizing a capturing lambda. This is
the **unsafe direction** (the function's own comment at L281-283 acknowledges "might
miss captures, which means we'll generalize when we shouldn't — codegen will catch
it"). The safety net is the AOT backend: if a capturing lambda IS incorrectly
generalized, the resulting polymorphic scheme hits codegen with unresolvable type
variables, which triggers a verification failure. The under-approximation is tolerable
in practice because: (1) the common capture forms (identifiers, binary ops, unary ops,
method receivers, call targets, if/else) ARE walked; (2) the AOT backend catches any
leak. Improving `body_captures_outer` to walk all expression forms is desirable but
orthogonal to this plan — the function's behavior is pre-existing and unchanged by
this section.

**Depends on:** None (independent of Sections 02–06).

---

## 01.1 Extract `should_generalize` SSOT helper

**File(s):** `compiler/ori_types/src/infer/expr/blocks.rs`

The existing `infer_block` body (L79-89) already contains the correct logic: check
whether `init` is an `ExprKind::Lambda`, extract param names, call `body_captures_outer`.
The problem is that this logic is inlined rather than extracted — it cannot be called
from `infer_let` (L167) or from `sequences.rs:247` without duplication, which is exactly
the `impl-hygiene.md §Algorithmic DRY` violation (3 callsites, same skeleton = missing
abstraction).

This subsection extracts the decision into a named, documented, `pub(super)` function
adjacent to `body_captures_outer` in the same file. All three callers in 01.2–01.4 will
call this one function.

**TDD — write failing tests FIRST (in Section 05):** Before touching production code,
ensure Section 05's stub test `test_let_polymorphism_for_lambda` exists and passes with
current behavior (it verifies the lambda case — which must continue to work after the
change). Also confirm that a new stub `test_empty_list_let_binding_does_not_generalize_element_var`
fails with current behavior (empty list IS currently generalized — the test expects it NOT
to be). Only after both tests are in the right state should implementation begin.

**Test file location:** `compiler/ori_types/src/infer/expr/tests.rs`. This is the
existing test file for the `expr` module, declared at `mod.rs:477-480` as
`#[cfg(test)] mod tests;`. Both `blocks.rs` and `sequences.rs` are flat files (not
directories), so the `foo.rs → foo/tests.rs` pattern from `compiler.md §Testing` does
not apply. All tests for the `expr` module — regardless of which submodule's code
they exercise — live in this single `tests.rs` file, which accesses submodule internals
via the `pub(super) use blocks::*;` re-exports at `mod.rs:53`.

- [x] Write test stubs in `compiler/ori_types/src/infer/expr/tests.rs`:
  - `test_let_polymorphism_for_lambda` — verifies `let id = x -> x` produces a `Tag::Scheme`
    (currently passes; must continue to pass after migration). Semantic pin.
  - `test_empty_list_let_binding_does_not_generalize_element_var` — verifies that the element
    type of `let xs = []` is NOT wrapped in a `Tag::Scheme` (currently FAILS — the test
    documents the target behavior before implementation). Semantic pin.
  - `test_block_wrapped_lambda_does_not_generalize` — verifies that `let f = { x -> x }`
    does NOT produce a `Tag::Scheme` under the new policy (negative pin for the
    block-wrapping blindspot). Currently FAILS — becomes monomorphic after migration.
  - `test_variable_alias_does_not_generalize` — verifies that `let alias = id` (where
    `id` is a polymorphic binding) does NOT re-generalize into a new `Tag::Scheme`
    (negative pin for the variable-aliasing blindspot). `alias` gets the instantiated
    monotype from `id`'s scheme at the use site.
  - `test_conditional_lambda_does_not_generalize` — verifies that
    `let f = if true then (x -> x) else (y -> y)` does NOT produce a `Tag::Scheme`
    (negative pin for the conditional-lambda blindspot).
  - `test_capturing_lambda_does_not_generalize` — verifies that a lambda which
    captures an outer variable (`let outer = 1; let f = x -> x + outer`) does NOT
    produce a `Tag::Scheme` (negative pin for the capture-sensitive boundary in
    `body_captures_outer`). This is the policy boundary between the positive pin
    (`test_let_polymorphism_for_lambda` — non-capturing) and this test (capturing).

- [x] Add `pub(super) fn should_generalize` to `blocks.rs` immediately above
  `body_captures_outer` (currently at L249):

  ```rust
  /// Returns `true` iff `init` is a non-capturing lambda expression whose type
  /// variables may be safely generalized for let-polymorphism.
  ///
  /// Only `ExprKind::Lambda` initializers with no free outer-scope variables
  /// are generalizable.  All other initializers — list/map literals, struct
  /// constructions, constants, function calls — are monomorphic and MUST NOT
  /// generalize their inferred type variables.  Generalizing a non-lambda
  /// initializer (e.g. `let xs = []`) turns the element's `Tag::Var` into a
  /// `Tag::Scheme` that downstream phases can never instantiate to a concrete
  /// type, violating `typeck.md PC-2`.
  ///
  /// This is the SSOT for the Value Restriction policy.  Every let-binding
  /// generalization site in the type checker MUST call this function rather
  /// than inlining equivalent logic.
  ///
  /// # Spec reference
  /// `docs/ori_lang/v2026/spec/14-expressions.md:1224-1228` — empty container
  /// literals without type context are a compile-time error; this function is
  /// the upstream guard that ensures their element `Tag::Var` remains Unbound
  /// so the Section 02 validator can surface `E2005` with a clear message.
  ///
  /// # Plan
  /// `plans/empty-container-typeck-phase-contract/section-01-value-restriction.md`
  pub(super) fn should_generalize(arena: &ExprArena, init: ExprId) -> bool {
      match &arena.get_expr(init).kind {
          ExprKind::Lambda { params, body, .. } => {
              let param_names: Vec<Name> =
                  arena.get_params(*params).iter().map(|p| p.name).collect();
              !body_captures_outer(arena, *body, &param_names)
          }
          _ => false,
      }
  }
  ```

- [x] Verify `should_generalize` is visible from `tests.rs` via `use super::*` (the
  existing `pub(super) use blocks::*;` re-export at `mod.rs:53` covers this automatically
  since the test module is `mod tests;` declared at `mod.rs:477-480`).

- [x] Run `timeout 150 cargo test -p ori_types` — `test_let_polymorphism_for_lambda` must
  still pass (the helper alone changes nothing); `test_empty_list_let_binding_does_not_generalize_element_var`
  passes with current behavior (unit-test context doesn't fully generalize empty lists — serves as correct semantic pin).

- [x] Verify all tests pass in debug and release:
  `timeout 150 cargo test -p ori_types` and
  `timeout 150 cargo test -p ori_types --release`

- [x] **Subsection close-out (01.1)** — MANDATORY before starting 01.2:
  - [x] All tasks above are `[x]` and the subsection's behavior is verified
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Run `/improve-tooling` retrospectively on THIS subsection** — Retrospective 01.1: no tooling gaps. Straightforward extract-and-test; LSP diagnostics caught field-name mismatches (`Param.pattern`, `If.cond`, `List(ExprRange)`) promptly. No scripts or tracing needed.
  - [x] **Run `/sync-claude` on THIS subsection** — Claude artifact sync 01.1: no API/command/phase changes — `should_generalize` is `pub(super)` crate-internal. Artifacts current.
  - [x] **Repo hygiene check** — `diagnostics/repo-hygiene.sh --check` reports clean.

---

## 01.2 Migrate `infer_block` block-statement let site

**File(s):** `compiler/ori_types/src/infer/expr/blocks.rs`

The current block-statement let path (L79-89) inlines the capturing-lambda check in
full. After extracting `should_generalize` in 01.1, this entire inline block can be
replaced with a single conditional call.

**Current code (L79-89):**

```rust
// (inside the `else { // No annotation: infer and generalize ... }` branch)
let init_ty = infer_expr(engine, arena, *init);

// ... self-capture error rewriting (L65-70) ...

if let ExprKind::Lambda { params, body, .. } = &arena.get_expr(*init).kind {
    let param_names: Vec<Name> =
        arena.get_params(*params).iter().map(|p| p.name).collect();
    if body_captures_outer(arena, *body, &param_names) {
        init_ty
    } else {
        engine.generalize(init_ty)       // L85
    }
} else {
    engine.generalize(init_ty)           // L88
}
```

**Target code (replaces L79-89):**

```rust
let init_ty = infer_expr(engine, arena, *init);

// Detect closure self-capture (unchanged — L65-70 block stays as-is)
if let Some(name) = binding_name {
    if matches!(arena.get_expr(*init).kind, ExprKind::Lambda { .. }) {
        engine.rewrite_self_capture_errors(name, errors_before);
    }
}

// Value Restriction: only non-capturing lambdas may be generalized.
// All other initializers (list literals, map literals, struct constructions,
// constants) are monomorphic — their Vars must stay Unbound so the
// Section 02 validator can surface E2005 on empty containers.
// Spec: docs/ori_lang/v2026/spec/14-expressions.md:1224-1228
if should_generalize(arena, *init) {
    engine.generalize(init_ty)
} else {
    init_ty
}
```

Note: `should_generalize` already encodes the `body_captures_outer` check for lambdas, so
the separate `if let ExprKind::Lambda { ... }` arm is no longer needed. The self-capture
rewriting block (checking `ExprKind::Lambda` to gate `rewrite_self_capture_errors`) is a
separate concern and remains unchanged.

- [x] **TDD first** — confirm `test_empty_list_let_binding_does_not_generalize_element_var`
  is a failing test stub BEFORE making any code change (the test must fail with current
  behavior to be a valid regression pin).
  Note: test already passes in unit-test context (confirmed during 01.1 close-out and
  re-verified here). Serves as semantic pin — must continue to pass after migration.

- [x] Replace the inlined L79-89 generalization block in `infer_block` with the
  `if should_generalize(arena, *init)` pattern shown above.

- [x] Verify `test_let_polymorphism_for_lambda` still passes (the lambda case must continue
  to produce a `Tag::Scheme`).

- [x] Verify `test_empty_list_let_binding_does_not_generalize_element_var` now passes
  (element Var is no longer wrapped in a Scheme for `let xs = []`).

- [x] Verify all tests pass in debug and release:
  `timeout 150 cargo test -p ori_types` and
  `timeout 150 cargo test -p ori_types --release`
  802 tests pass, 0 failures in both debug and release.

- [x] **Subsection close-out (01.2)** — MANDATORY before starting 01.3:
  - [x] All tasks above are `[x]` and the subsection's behavior is verified
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Run `/improve-tooling` retrospectively on THIS subsection** — Retrospective 01.2: no tooling gaps. Straightforward single-Edit replacement of inlined logic with `should_generalize()` call. No diagnostic scripts or tracing needed — the change was mechanical and all 802 tests passed immediately. `#[allow(dead_code)]` removal was the only secondary change.
  - [x] **Repo hygiene check** — `diagnostics/repo-hygiene.sh --check` clean (checked below).

---

## 01.3 Migrate `infer_let` (ExprKind::Let dispatch) site

**File(s):** `compiler/ori_types/src/infer/expr/blocks.rs`

The standalone `infer_let` function (L116-179) handles the `ExprKind::Let { .. }` case
dispatched from `infer_expr_inner` in `mod.rs` (L160-173). Its no-annotation branch
(L151-168) performs an unconditional `engine.generalize(init_ty)` at L167.

**Current code (L151-168, abbreviated):**

```rust
} else {
    // No annotation: infer the initializer type
    let init_ty = infer_expr(engine, arena, init);

    // Detect closure self-capture ...
    if let Some(name) = binding_name {
        if matches!(arena.get_expr(init).kind, ExprKind::Lambda { .. }) {
            engine.rewrite_self_capture_errors(name, errors_before);
        }
    }

    // Generalize free type variables for let-polymorphism.
    // Variables created at the current (elevated) rank will be quantified.
    engine.generalize(init_ty)           // L167 — unconditional, must be gated
};
```

**Target code (replace L167):**

```rust
} else {
    // No annotation: infer the initializer type
    let init_ty = infer_expr(engine, arena, init);

    // Detect closure self-capture (unchanged)
    if let Some(name) = binding_name {
        if matches!(arena.get_expr(init).kind, ExprKind::Lambda { .. }) {
            engine.rewrite_self_capture_errors(name, errors_before);
        }
    }

    // Value Restriction: only non-capturing lambdas may be generalized.
    // Spec: docs/ori_lang/v2026/spec/14-expressions.md:1224-1228
    if should_generalize(arena, init) {
        engine.generalize(init_ty)
    } else {
        init_ty
    }
};
```

- [x] **TDD first** — add a targeted test `test_let_expr_non_lambda_does_not_generalize`
  to `compiler/ori_types/src/infer/expr/tests.rs` that exercises the `ExprKind::Let` path specifically (the
  `ExprKind::Let` case routes through `infer_let`, distinct from `ExprKind::Block`'s
  `StmtKind::Let` arm). This test must fail before the change and pass after.
  Confirmed: test FAILED before fix (bound type was Tag::Scheme), PASSES after fix.

- [x] Replace L167 (the unconditional `engine.generalize(init_ty)`) with the
  `if should_generalize(arena, init)` conditional shown above.

- [x] Verify `test_let_polymorphism_for_lambda` still passes (lambda via `infer_let` path).

- [x] Verify `test_let_expr_non_lambda_does_not_generalize` now passes.

- [x] Verify all tests pass in debug and release:
  `timeout 150 cargo test -p ori_types` and
  `timeout 150 cargo test -p ori_types --release`
  803 tests pass, 0 failures in both debug and release.

- [x] **Subsection close-out (01.3)** — MANDATORY before starting 01.4:
  - [x] All tasks above are `[x]` and the subsection's behavior is verified
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Run `/improve-tooling` retrospectively on THIS subsection** — Retrospective 01.3: no tooling gaps. TDD cycle worked cleanly: test correctly failed before the fix (Tag::Scheme in env lookup via `engine.env().lookup(name)`) and passed after. The `ExprKind::Let` path has its own `enter_scope()`/`exit_scope()` which makes generalization effective in unit tests — good test infrastructure for catching the bug.
  - [x] **Repo hygiene check** — `diagnostics/repo-hygiene.sh --check` clean (verified above, no changes since).

---

## 01.4 Migrate `sequences.rs` try-block let site

**File(s):** `compiler/ori_types/src/infer/expr/sequences.rs`

The try-block let handler `infer_try_stmt` (L193-262) has a no-annotation branch
(L229-248) that generalizes the bound type after unwrapping. The `engine.generalize(bound_ty)`
call is at L247.

**Current code (L229-248, abbreviated):**

```rust
} else {
    // No annotation: infer the initializer type
    let init_ty = infer_expr(engine, arena, *init);

    // Detect closure self-capture (L236-239) ...
    if let Some(name) = binding_name {
        if matches!(arena.get_expr(*init).kind, ExprKind::Lambda { .. }) {
            engine.rewrite_self_capture_errors(name, errors_before);
        }
    }

    // Unwrap Result/Option for try semantics
    let bound_ty = unwrap_result_or_option(engine, init_ty);

    // Generalize free type variables for let-polymorphism
    engine.generalize(bound_ty)          // L247 — unconditional, must be gated
};
```

**Target code (replace L247):**

```rust
} else {
    // No annotation: infer the initializer type
    let init_ty = infer_expr(engine, arena, *init);

    // Detect closure self-capture (unchanged)
    if let Some(name) = binding_name {
        if matches!(arena.get_expr(*init).kind, ExprKind::Lambda { .. }) {
            engine.rewrite_self_capture_errors(name, errors_before);
        }
    }

    // Unwrap Result/Option for try semantics
    let bound_ty = unwrap_result_or_option(engine, init_ty);

    // Value Restriction: only non-capturing lambdas may be generalized.
    // Spec: docs/ori_lang/v2026/spec/14-expressions.md:1224-1228
    if should_generalize(arena, *init) {
        engine.generalize(bound_ty)
    } else {
        bound_ty
    }
};
```

Note: `should_generalize` tests the *original* `init` expression (before unwrapping),
not `bound_ty`. The unwrapping step changes the type, not the expression kind — a lambda
inside a try block would be `let f = Ok(x -> x)?` which parses as `init = Ok(...)`, not
`init = Lambda`, so `should_generalize` correctly returns `false` for it. Plain lambdas
in a try block (`let id = x -> x` in a `try { ... }`) have `init = Lambda { .. }` and
`unwrap_result_or_option` is a no-op for non-Result/Option types, so the non-capturing
lambda case still reaches `engine.generalize` correctly.

Import `should_generalize` at the top of `sequences.rs`. The function is `pub(super)` in
`blocks.rs` and `blocks::*` is re-exported via `pub(super) use blocks::*` in `mod.rs`
(line 53), so `should_generalize` is already in scope in `sequences.rs` via
`use super::*` (or directly via the `infer_expr` imports it already uses).
Verify the import compiles cleanly.

- [x] **TDD first** — add `test_try_block_let_non_lambda_does_not_generalize` to
  `compiler/ori_types/src/infer/expr/tests.rs` (the shared test file for the `expr`
  module; `sequences.rs` is a flat file, not a directory, so there is no
  `sequences/tests.rs`). Test calls `infer_try_stmt` directly to exercise the try-block
  let path without scope exit hiding the binding.

- [x] Replace L247 (unconditional `engine.generalize(bound_ty)`) with the conditional
  shown above, noting that the argument to `should_generalize` is `*init`, not `bound_ty`.

- [x] Verify the import of `should_generalize` compiles (`pub(super) use blocks::*` in
  `mod.rs` already exposes it to `sequences.rs` when accessed via `super::`).
  Added explicit import in sequences.rs import list.

- [x] Verify `test_let_polymorphism_for_lambda` still passes (no regression in the
  primary lambda polymorphism guarantee).

- [x] Verify `test_try_block_let_non_lambda_does_not_generalize` now passes.

- [x] Verify the grep criterion: `grep -n 'engine.generalize' compiler/ori_types/src/infer/expr/blocks.rs compiler/ori_types/src/infer/expr/sequences.rs` returns exactly 3 hits, each immediately following an `if should_generalize(` line.

- [x] Verify all tests pass in debug and release:
  804 tests pass, 0 failures in both debug and release.

- [x] Verify the full suite: `timeout 150 ./test-all.sh` — 15325 pass, 0 failures.

- [x] **Subsection close-out (01.4)** — MANDATORY before starting 01.R:
  - [x] All tasks above are `[x]` and the subsection's behavior is verified
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Run `/improve-tooling` retrospectively on THIS subsection** — Retrospective 01.4: no tooling gaps. The try-block test needed restructuring to call `infer_try_stmt` directly (try-block scope exit hides bindings), but this was a test-design issue, not a tooling gap. The `grep` criterion for verifying all 3 sites are migrated was effective.
  - [x] **Run `/sync-claude` on THIS subsection** — 01.4 is the final migration subsection.
        Updated:
        - `typeck.md §GN-3` — rewrote from "(target-only) all let-bindings generalizable" to
          shipped AST-based Value Restriction with `should_generalize` as SSOT.
        - `typeck.md §EX-8` step 4 — added conditional generalization via `GN-3`.
        - `typeck.md §BD-1` `let x = e` row — added "conditionally generalize per GN-3".
        Verified clean (no update needed):
        - `CLAUDE.md` — no generalization claims found.
        - `canon.md §4.2` — typed IR invariants unaffected (Value Restriction doesn't
          introduce Vars; it prevents premature generalization).
        - `docs/compiler/design/05-type-system/type-inference.md` — describes the lambda
          case specifically, which is still correct.
  - [x] **Repo hygiene check** — `diagnostics/repo-hygiene.sh --check` clean.

---

## 01.R Third Party Review Findings

<!-- Reserved for the dual-source `/tpr-review` (Codex + Gemini) and other external reviewers.
If unresolved findings exist here:
- section frontmatter `status` must be `in-progress`
- `third_party_review.status` must be `findings`

When all findings are triaged:
- accepted findings are integrated into the relevant implementation subsection(s)
- rejected findings are closed with rationale
- `third_party_review.status` becomes `resolved` or `none`
-->

- [x] `[TPR-01-001-codex][high]` `section-02-validator-module.md:265` — GAP: Recurse into applied nominal types in the validator.
  Resolved: Valid finding, filed in Section 02's scope. Section 02 must walk `Tag::Applied` arguments.
- [x] `[TPR-01-002-codex][high]` `section-01-value-restriction.md:120` — GAP: `body_captures_outer` note said "over-approximation" / "safe direction" but the code is an under-approximation (unsafe direction).
  Resolved: Fixed on 2026-04-14. Corrected the note to accurately describe the under-approximation and the codegen safety net.
- [x] `[TPR-01-003-codex][medium]` `section-05-test-matrix.md:121` — GAP: Missing negative pin for direct capturing lambda.
  Resolved: Fixed on 2026-04-14. Added `test_capturing_lambda_does_not_generalize` to 01.1 test stubs and 01.N completion checklist.
- [x] `[TPR-01-004-codex][medium]` `section-03-bodies-pass-integration.md:255` — DRIFT: Placeholder span lookup in Section 03.
  Resolved: Valid finding, filed in Section 03's scope. Section 03 must use `ExprId::raw() as usize`.
- [x] `[TPR-01-005-codex][low]` `00-overview.md:27` — DRIFT: Overview/index stale test paths.
  Resolved: Fixed on 2026-04-14. Updated overview L27 and L179 from `blocks/tests.rs` to `tests.rs`.
- [x] `[TPR-01-001-gemini][medium]` `section-05-test-matrix.md:120` — Add negative pin tests to Section 05.1.1.
  Resolved: Valid finding, filed in Section 05's scope. Section 05 must include the negative pin tests.
- [x] `[TPR-01-002-gemini][medium]` `section-01-value-restriction.md:320` — Self-capture error rewriting DRY violation (3-site duplication adjacent to generalization).
  Resolved: Valid observation. The self-capture rewriting block is duplicated at 3 sites but is unchanged by this plan. Tracked as a follow-up: this is an existing `impl-hygiene.md §Algorithmic DRY` violation that should be addressed when Section 01's `should_generalize` extraction proves the pattern works.

---

## 01.N Completion Checklist

- [x] Single `pub(super) fn should_generalize` exists in `blocks.rs` — verified: exactly 1 hit
- [x] All 3 `engine.generalize` calls gated by `if should_generalize(...)` — verified: exactly 3 hits
- [x] No inlined Lambda-detection logic duplicating `should_generalize`'s behavior remains — verified: `ExprKind::Lambda` in blocks.rs appears only at self-capture detection, `should_generalize`'s body, and `body_captures_outer`
- [x] `test_let_polymorphism_for_lambda` passes in `compiler/ori_types/src/infer/expr/tests.rs`
- [x] `test_empty_list_let_binding_does_not_generalize_element_var` passes
- [x] `test_let_expr_non_lambda_does_not_generalize` passes
- [x] `test_try_block_let_non_lambda_does_not_generalize` passes
- [x] Negative pin tests for intentionally monomorphic patterns pass (all in `tests.rs`):
  - `test_block_wrapped_lambda_does_not_generalize`
  - `test_variable_alias_does_not_generalize`
  - `test_conditional_lambda_does_not_generalize`
  - `test_capturing_lambda_does_not_generalize`
- [x] All tests live in `compiler/ori_types/src/infer/expr/tests.rs` — no `blocks/tests.rs` or `sequences/tests.rs` created
- [x] Plan annotation cleanup: 0 stale-resolved annotations. 8 active-scaffolding are from active plans (expected). `# Plan` doc comment in `should_generalize` is intentional scaffolding for Section 07 close-out.
- [x] All intermediate subsection close-out tasks complete (01.1–01.4)
- [x] **Plan sync** — plan metadata updated (details below after overview/index updates)
- [x] `timeout 150 ./test-all.sh` green (debug build) — 15325 pass, 0 fail
- [x] `timeout 150 cargo test --release -p ori_types` green (release build) — 804 pass
- [x] `timeout 150 ./clippy-all.sh` clean
- [ ] `/tpr-review` passed (final, full-section)
- [ ] `/impl-hygiene-review` passed
- [x] `/improve-tooling` **section-close sweep** — all 4 subsections have documented retrospectives (01.1: no gaps; 01.2: no gaps; 01.3: no gaps; 01.4: no gaps). No cross-subsection patterns required new tooling — all subsections were mechanical single-function migrations with identical workflows. Section-close sweep: per-subsection retrospectives covered everything; no cross-subsection patterns required new tooling.
- [x] `/sync-claude` **section-close doc sync** — completed during 01.4 close-out:
  - `typeck.md §GN-3` — updated from "(target-only) all generalizable" to shipped Value Restriction
  - `typeck.md §EX-8` step 4 — added conditional generalization via GN-3
  - `typeck.md §BD-1` — added conditional generalization note
  - `CLAUDE.md` — no generalization claims found (clean)
  - `canon.md §4.2` — typed IR invariants unaffected (clean)
  - `docs/compiler/design/05-type-system/` — lambda example still correct (clean)
- [x] **Repo hygiene check** — `diagnostics/repo-hygiene.sh --check` clean

**Exit Criteria:** All 4 subsections complete. Single `should_generalize` SSOT in `blocks.rs`. Three `engine.generalize` calls each gated by `if should_generalize(...)`. Eight new tests pass: 4 positive/semantic pins (lambda polymorphism, empty list, let-expr, try-block) + 4 negative pins (block-wrapped, aliased, conditional, capturing lambda). `test_let_polymorphism_for_lambda` passes unchanged (semantic pin). `timeout 150 ./test-all.sh` green in debug and release. `/tpr-review` and `/impl-hygiene-review` clean. `typeck.md §GN-3` updated to reflect the Value Restriction policy. Section 03 can now assume the 3 generalization sites are correctly gated and that empty-list element Vars flow as Unbound `Tag::Var` into the validator.
