---
section: "01"
title: "AST-based Value Restriction"
status: not-started
reviewed: false
goal: "Extract a single SSOT `should_generalize` helper and migrate all 3 let-generalization sites to call it, so non-lambda initializers (including empty lists) no longer generalize their element type variables."
success_criteria:
  - "Single `pub(super) fn should_generalize(arena: &ExprArena, init: ExprId) -> bool` exists in `compiler/ori_types/src/infer/expr/blocks.rs` — verifiable via `grep -n 'pub(super) fn should_generalize' compiler/ori_types/src/infer/expr/blocks.rs` returning exactly one hit."
  - "All 3 generalization sites call `should_generalize` — NOT a type-tag heuristic, NOT inlined duplicated logic. Verifiable: `grep -n 'engine.generalize' compiler/ori_types/src/infer/expr/blocks.rs compiler/ori_types/src/infer/expr/sequences.rs` returns exactly 3 call sites each preceded by `if should_generalize(...)`."
  - "`let id = x -> x; id(1); id(\"hello\")` type-checks and runs correctly in both interpreter and LLVM — regression pin for let-polymorphism preservation. Test `test_let_polymorphism_for_lambda` in `compiler/ori_types/src/infer/expr/blocks/tests.rs` passes BEFORE and AFTER the change; reverting `should_generalize` must break it."
  - "`let x = []` no longer generalizes the element Var — `Tag::Var` stays Unbound after the block-statement let path returns, ready for Section 02's validator to catch. Verifiable via a unit test `test_empty_list_let_binding_does_not_generalize_element_var` in the same tests file."
  - "`timeout 150 ./test-all.sh` remains green (debug and release builds) after the migration — no regressions in existing spec tests."
inspired_by:
  - "Rust `rustc_hir_typeck` — no let-polymorphism for local bindings; all local bindings are monomorphic. Every `let x = e` in a function body constrains `e`'s type variable to the inferred monotype rather than generalizing it."
  - "Haskell monomorphism restriction — motivation for Value Restriction: unrestricted generalization of mutable or effectful bindings leads to unsoundness; even in a pure setting, generalizing container element types produces unresolvable Vars downstream."
  - "Ori `body_captures_outer` precedent at `compiler/ori_types/src/infer/expr/blocks.rs:79-89` — the codebase ALREADY uses AST-based Lambda detection to distinguish non-capturing from capturing closures. `should_generalize` extends this exact pattern to the generalization decision itself."
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "01.1"
    title: "Extract `should_generalize` SSOT helper"
    status: not-started
  - id: "01.2"
    title: "Migrate `infer_block` block-statement let site"
    status: not-started
  - id: "01.3"
    title: "Migrate `infer_let` (ExprKind::Let dispatch) site"
    status: not-started
  - id: "01.4"
    title: "Migrate `sequences.rs` try-block let site"
    status: not-started
  - id: "01.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "01.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 01: AST-based Value Restriction

**Status:** Not Started
**Goal:** Replace unconditional generalization at 3 let-binding sites with a single SSOT
`should_generalize(arena, init_expr_id) -> bool` helper that returns `true` only for
non-capturing `ExprKind::Lambda` initializers. Preserves let-polymorphism for
`let id = x -> x` while preventing empty-list element Vars from being prematurely
generalized into Schemes.

**Success Criteria:**

- [ ] Single `pub(super) fn should_generalize` exists in `blocks.rs` — one grep hit
- [ ] All 3 `engine.generalize()` calls are gated by `if should_generalize(...)` — grep verifiable
- [ ] `test_let_polymorphism_for_lambda` passes before and after; reverting the change breaks it
- [ ] `test_empty_list_let_binding_does_not_generalize_element_var` passes after migration
- [ ] `timeout 150 ./test-all.sh` green (debug + release)

**Context:** BUG-04-074 traced the "unresolved type variable at codegen" failure to three
unconditional `engine.generalize()` calls in the typeck let-binding paths. Generalizing
an empty-list element's `Tag::Var` turns it into a `Tag::Scheme` whose bound var is
never instantiated to a concrete type — downstream use sites like `.len()` don't constrain
the element type, so the Scheme persists unresolved through canonicalization, ARC lowering,
and into LLVM codegen where it triggers a verification failure. The fix is AST-based Value
Restriction: only non-capturing lambdas (`x -> x`) qualify for generalization; all other
initializers — including `[]`, `{}`, struct literals, and constants — are monomorphic and
must not generalize their type variables.

**Reference implementations:**

- **Rust** `compiler/rustc_hir_typeck/src/expr.rs`: no let-polymorphism for local let
  bindings — Rust's type checker never generalizes locally-bound types into schemes; every
  `let x = e` in a function body is monomorphic. Ori's design differs (it supports
  `let id = x -> x` with genuine polymorphism for non-capturing lambdas), but the lesson
  is clear: unrestricted generalization of arbitrary initializers is unsound.

- **Haskell** `ghc/compiler/GHC/Tc/Gen/Bind.hs`: the monomorphism restriction
  motivates why even a purely functional language needs Value Restriction — functions
  defined without explicit type signatures that involve type classes can behave
  unexpectedly when generalized and re-used at different types.

- **Ori** `compiler/ori_types/src/infer/expr/blocks.rs:79-89`: `body_captures_outer`
  precedent — the codebase already performs AST-based Lambda detection + capture analysis
  to decide whether a lambda is capturing. `should_generalize` extends this exact check
  to make generalization conditional on the result.

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

- [ ] Write test stubs in `compiler/ori_types/src/infer/expr/blocks/tests.rs`
  (create the `tests.rs` sibling file if it does not yet exist — follow the
  `blocks.rs` → `blocks/tests.rs` pattern from `compiler.md §Testing`):
  - `test_let_polymorphism_for_lambda` — verifies `let id = x -> x` produces a `Tag::Scheme`
    (currently passes; must continue to pass after migration)
  - `test_empty_list_let_binding_does_not_generalize_element_var` — verifies that the element
    type of `let xs = []` is NOT wrapped in a `Tag::Scheme` (currently FAILS — the test
    documents the target behavior before implementation)

- [ ] Add `pub(super) fn should_generalize` to `blocks.rs` immediately above
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

- [ ] Verify `should_generalize` is visible from sibling `tests.rs` via `use super::*` (the
  existing `pub(super) use blocks::*;` re-export in `mod.rs` covers this automatically).

- [ ] Run `timeout 150 cargo test -p ori_types` — `test_let_polymorphism_for_lambda` must
  still pass (the helper alone changes nothing); `test_empty_list_let_binding_does_not_generalize_element_var`
  remains failing (expected — implementation comes in 01.2–01.4).

- [ ] Verify all tests pass in debug and release:
  `timeout 150 cargo test -p ori_types` and
  `timeout 150 cargo test -p ori_types --release`

- [ ] **Subsection close-out (01.1)** — MANDATORY before starting 01.2:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — reflect on the
        debugging journey for 01.1 specifically: which `diagnostics/` scripts you ran,
        where you added `dbg!`/`tracing` calls, where output was hard to interpret, where
        test failures gave unhelpful messages, where you ran the same command sequence
        repeatedly. Forward-look: what tool/log/diagnostic would shorten the next
        regression in 01.1's code path by 10 minutes? Implement every accepted improvement
        NOW (zero deferral) and commit each via SEPARATE `/commit-push` (e.g.,
        `build(diagnostics): add X to Y.sh — surfaced by empty-container-contract/section-01.1
        retrospective`). Use a valid conventional-commit type — `build` for dev/diagnostic
        scripts, `test` for test-harness changes, `chore` for general tooling, `ci` for CI
        config, `docs` for tool docs. Do NOT use `tools(...)` — the lefthook commit-msg hook
        rejects any type outside the standard set. Mandatory even when nothing felt painful.
        If genuinely no gaps, document: "Retrospective 01.1: no tooling gaps." Do not
        silently skip. See `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection
        Workflow" for the full protocol.
  - [ ] **Run `/sync-claude` on THIS subsection** — check whether the code changes in 01.1
        invalidated any claims in CLAUDE.md, `.claude/rules/*.md`, or `canon.md`. Three
        quick questions: (1) Did I add/rename/remove any public API, type, variant, or
        function? → Check the relevant rules file. (2) Did I add/change any command, env var,
        or script? → Check CLAUDE.md §Commands. (3) Did I change any pipeline phase behavior
        or output invariant? → Check `canon.md`. If all three are "no," document: "Claude
        artifact sync 01.1: no API/command/phase changes — artifacts current." Fix any drift
        NOW and commit via `/commit-push`. Do not silently skip.
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any
        temp/scratch files that accumulated during this subsection. If files are found, run
        `diagnostics/repo-hygiene.sh --clean` to remove them.

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

- [ ] **TDD first** — confirm `test_empty_list_let_binding_does_not_generalize_element_var`
  is a failing test stub BEFORE making any code change (the test must fail with current
  behavior to be a valid regression pin).

- [ ] Replace the inlined L79-89 generalization block in `infer_block` with the
  `if should_generalize(arena, *init)` pattern shown above.

- [ ] Verify `test_let_polymorphism_for_lambda` still passes (the lambda case must continue
  to produce a `Tag::Scheme`).

- [ ] Verify `test_empty_list_let_binding_does_not_generalize_element_var` now passes
  (element Var is no longer wrapped in a Scheme for `let xs = []`).

- [ ] Verify all tests pass in debug and release:
  `timeout 150 cargo test -p ori_types` and
  `timeout 150 cargo test -p ori_types --release`

- [ ] **Subsection close-out (01.2)** — MANDATORY before starting 01.3:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — same protocol as
        01.1's close-out, scoped to 01.2's debugging journey. Commit improvements
        separately using a valid conventional-commit type:
        `build(diagnostics): ... — surfaced by empty-container-contract/section-01.2
        retrospective` (or `test(...)`, `chore(...)`, etc — see 01.1's close-out for type
        rules).
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any
        detected temp files (see 01.1's close-out for details).

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

- [ ] **TDD first** — add a targeted test `test_let_expr_non_lambda_does_not_generalize`
  to `blocks/tests.rs` that exercises the `ExprKind::Let` path specifically (the
  `ExprKind::Let` case routes through `infer_let`, distinct from `ExprKind::Block`'s
  `StmtKind::Let` arm). This test must fail before the change and pass after.

- [ ] Replace L167 (the unconditional `engine.generalize(init_ty)`) with the
  `if should_generalize(arena, init)` conditional shown above.

- [ ] Verify `test_let_polymorphism_for_lambda` still passes (lambda via `infer_let` path).

- [ ] Verify `test_let_expr_non_lambda_does_not_generalize` now passes.

- [ ] Verify all tests pass in debug and release:
  `timeout 150 cargo test -p ori_types` and
  `timeout 150 cargo test -p ori_types --release`

- [ ] **Subsection close-out (01.3)** — MANDATORY before starting 01.4:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — same protocol as
        01.1's close-out, scoped to 01.3's debugging journey. Commit improvements
        separately using a valid conventional-commit type:
        `build(diagnostics): ... — surfaced by empty-container-contract/section-01.3
        retrospective`.
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any
        detected temp files.

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

- [ ] **TDD first** — add `test_try_block_let_non_lambda_does_not_generalize` to
  `compiler/ori_types/src/infer/expr/sequences/tests.rs` (create the sibling file if
  absent — `sequences.rs` → `sequences/tests.rs`). Test must fail before the change and
  pass after.

- [ ] Replace L247 (unconditional `engine.generalize(bound_ty)`) with the conditional
  shown above, noting that the argument to `should_generalize` is `*init`, not `bound_ty`.

- [ ] Verify the import of `should_generalize` compiles (`pub(super) use blocks::*` in
  `mod.rs` already exposes it to `sequences.rs` when accessed via `super::`).

- [ ] Verify `test_let_polymorphism_for_lambda` still passes (no regression in the
  primary lambda polymorphism guarantee).

- [ ] Verify `test_try_block_let_non_lambda_does_not_generalize` now passes.

- [ ] Verify the grep criterion: `grep -n 'engine.generalize' compiler/ori_types/src/infer/expr/blocks.rs compiler/ori_types/src/infer/expr/sequences.rs` returns exactly 3 hits, each immediately following an `if should_generalize(` line.

- [ ] Verify all tests pass in debug and release:
  `timeout 150 cargo test -p ori_types` and
  `timeout 150 cargo test -p ori_types --release`

- [ ] Verify the full suite: `timeout 150 ./test-all.sh`

- [ ] **Subsection close-out (01.4)** — MANDATORY before starting 01.R:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — same protocol as
        01.1's close-out, scoped to 01.4's debugging journey. Commit improvements
        separately using a valid conventional-commit type:
        `build(diagnostics): ... — surfaced by empty-container-contract/section-01.4
        retrospective`.
  - [ ] **Run `/sync-claude` on THIS subsection** — 01.4 is the final migration subsection.
        After 01.4, the generalization policy has changed across all 3 sites. Check whether
        `typeck.md §GN-1` / `§GN-3` or `CLAUDE.md §Type Checker Patterns` need updating to
        document the new Value Restriction policy. If `typeck.md §GN-3` (currently states
        "all let-bindings are generalizable") now diverges from the shipped behavior, update
        it: the rule is now "only non-capturing lambda initializers are generalizable for
        local let bindings." If all three are "no," document: "Claude artifact sync 01.4:
        updated typeck.md §GN-3 to reflect Value Restriction shipping." Fix any drift NOW
        and commit via `/commit-push`. Do not silently skip.
  - [ ] **Repo hygiene check** — run `diagnostics/repo-hygiene.sh --check` and clean any
        detected temp files.

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

- None.

---

## 01.N Completion Checklist

- [ ] Single `pub(super) fn should_generalize` exists in `blocks.rs` — `grep -n 'pub(super) fn should_generalize' compiler/ori_types/src/infer/expr/blocks.rs` returns exactly one hit
- [ ] All 3 `engine.generalize` calls gated by `if should_generalize(...)` — `grep -n 'engine.generalize' compiler/ori_types/src/infer/expr/blocks.rs compiler/ori_types/src/infer/expr/sequences.rs` returns exactly 3 hits
- [ ] No inlined Lambda-detection logic duplicating `should_generalize`'s behavior remains — `grep -n 'ExprKind::Lambda' compiler/ori_types/src/infer/expr/blocks.rs` shows only the self-capture detection arm and `should_generalize`'s own body
- [ ] `test_let_polymorphism_for_lambda` passes in `compiler/ori_types/src/infer/expr/blocks/tests.rs`
- [ ] `test_empty_list_let_binding_does_not_generalize_element_var` passes
- [ ] `test_let_expr_non_lambda_does_not_generalize` passes
- [ ] `test_try_block_let_non_lambda_does_not_generalize` passes in `compiler/ori_types/src/infer/expr/sequences/tests.rs`
- [ ] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan 01` returns 0 ephemeral annotations — the `# Plan` doc comment in `should_generalize` referencing this section is intentional scaffolding to be removed at Section 07 close-out (per `00-overview.md §Known Bugs` close-out note)
- [ ] All intermediate subsection close-out tasks complete (01.1–01.4)
- [ ] **Plan sync** — update plan metadata to reflect section completion:
  - [ ] This section's frontmatter `status` → `complete`, all subsection statuses updated
  - [ ] `00-overview.md` Quick Reference table entry for Section 01 updated to `Complete`
  - [ ] `00-overview.md` mission success criteria: check off the let-polymorphism regression criterion if now satisfied
  - [ ] `index.md` section status updated
  - [ ] Section 03's `depends_on` references Section 01 — verify Section 03's assumptions still hold after this change (specifically: that the 3 sites are now calling `should_generalize` and not inlining)
- [ ] `timeout 150 ./test-all.sh` green (debug build)
- [ ] `timeout 150 cargo test --release -p ori_types` green (release build)
- [ ] `timeout 150 ./clippy-all.sh` clean
- [ ] `/tpr-review` passed (final, full-section) — independent dual-source review (Codex + Gemini) found no critical or major issues (or all findings from both reviewers triaged and recorded in 01.R)
- [ ] `/impl-hygiene-review` passed — implementation hygiene review found no critical or major findings (or all findings triaged and fixed). MUST run AFTER `/tpr-review` is clean.
- [ ] `/improve-tooling` **section-close sweep** — verify every subsection (01.1–01.4) has either an "improvements made" entry (with commits) or a documented "no gaps" negative finding from its per-subsection retrospective. Look for cross-subsection patterns invisible at per-item scope. Add only new items from cross-cutting patterns; implement immediately, commit separately. If no new patterns found, document: "Section-close sweep: per-subsection retrospectives covered everything; no cross-subsection patterns required new tooling."
- [ ] `/sync-claude` **section-close doc sync** — run across all commits in Section 01 (`git diff --name-only <section-start>..HEAD`). Map changed files to rules (primarily `typeck.md §GN-3` for the Value Restriction policy change). Verify CLAUDE.md §Type Checker Patterns still accurate. Fix any drift and commit. Document result.
- [ ] **Repo hygiene check** — `diagnostics/repo-hygiene.sh --check` clean before final commit

**Exit Criteria:** All 4 subsections complete. Single `should_generalize` SSOT in `blocks.rs`. Three `engine.generalize` calls each gated by `if should_generalize(...)`. Four new tests pass. `test_let_polymorphism_for_lambda` passes unchanged (semantic pin). `timeout 150 ./test-all.sh` green in debug and release. `/tpr-review` and `/impl-hygiene-review` clean. Section 03 can now assume the 3 generalization sites are correctly gated and that empty-list element Vars flow as Unbound `Tag::Var` into the validator.
