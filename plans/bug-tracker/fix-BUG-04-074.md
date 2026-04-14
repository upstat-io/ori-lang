---
bug: "BUG-04-074"
title: "AOT codegen: empty list literal `[]` with `push()` leaves unresolved type variables — LLVM verification failure"
severity: "high"
status: in-progress
goal: "Empty container literals (`[]`, `Set<T>`, `{}`) with element types inferred solely from downstream usage compile cleanly through AOT, with `resolve_fully()` producing concrete element types for codegen."
success_criteria:
  - "The exact repro `let ages = []; ages = ages.push(value: 10); if ages.len() == 1 then 0 else 1` compiles via `ori build` and runs successfully"
  - "No `unresolved type variable at codegen` errors from `ori_llvm::codegen::type_info::store` for empty-list-with-inferred-element cases"
  - "Interpreter and LLVM produce identical results (dual-execution parity) for the repro and edge cases"
  - "Matrix tests cover empty `[]`, `Set<T>`, `{}` + int/str/bool/struct element types + push/insert/len/iter usage patterns"
  - "No regressions in `timeout 150 ./test-all.sh`"
subsystem: "compiler/ori_types/src/infer/expr/blocks.rs"
found: "2026-04-13"
source: "continue-roadmap"
third_party_review:
  status: none
  updated: null
---

# Fix: BUG-04-074 — AOT codegen: empty list literal `[]` with `push()` leaves unresolved type variables — LLVM verification failure

**Status:** In Progress
**Severity:** high
**Goal:** Empty container literals without explicit type annotations must produce resolvable types at codegen time when constrained by downstream usage within the same function body.

**Success Criteria:**
- [ ] Repro `let ages = []; ages = ages.push(value: 10); if ages.len() == 1 then 0 else 1` compiles via `ori build` and exits 0
- [ ] Matrix of empty-container + usage-constraint combinations compiles and runs through both interpreter and LLVM
- [ ] Semantic pin test that only passes with the new resolution behavior
- [ ] Negative pin that rejects the old generalized-var-leak behavior
- [ ] No regressions in `timeout 150 ./test-all.sh`

**Context:** Filed 2026-04-13 during continue-roadmap work. The interpreter handles empty lists correctly (type inference flows through naturally), but AOT compilation fails with three `unresolved type variable at codegen — type inference bug` errors. The bug is intermittent across empty-container scenarios and blocks AOT compilation of idiomatic Ori code like initializing an empty list and populating it via `push()`.

---

## 1. Root Cause Analysis

- **Symptom**: `ori build` emits `error[E5001]: LLVM module verification failed` with 3 preceding `unresolved type variable at codegen` errors on Idx(96), Idx(99), Idx(103). The interpreter (`ori run`) handles the same program correctly.

- **Proximate cause**: The stored expression type for the empty list literal `[]` in `TypedModule.expr_types` is `List(Var(X))` where `Var(X)` has state `Generalized` and has NO `Link` to a concrete type. Additional instantiation vars (Var(Y)) at use sites like `ages.len()` remain `Unbound` because `.len()` doesn't constrain the element type. When `ori_llvm::codegen::type_info::store::get_or_compute_type_info` encounters these Vars during codegen emission, `pool.resolve_fully(idx)` returns the Var unchanged (because it only follows `VarState::Link` chains, not `Generalized` or `Unbound` states), triggering the error path.

- **Root cause**: The `infer_let` function in `compiler/ori_types/src/infer/expr/blocks.rs:167` unconditionally calls `engine.generalize(init_ty)` on every let binding initializer type, implementing HM let-polymorphism. For empty containers whose element type is a fresh unification variable introduced by `infer_empty_list()`, generalization marks the element var as `VarState::Generalized`. Later scheme instantiations at use sites create fresh `Var(Y)`s — SOME get linked (e.g., `push(value: 10)` unifies its fresh var with `int`), but OTHERS at element-type-irrelevant use sites (like `.len()`) stay `Unbound`. There is no persistent mapping from the generalized var to its concrete instantiations, so the expression type stored on the `[]` literal retains the generalized var, and the `.len()` call's instantiation var retains its `Unbound` state.

  **Confirmed by debug output**:
  - `Idx(96)`: `Var(5)` state = `Generalized { id: 5, name: None }` — the original empty-list element var
  - `Idx(99)`: `Var(6)` state = `Unbound { id: 6, rank: Rank(3), name: None }` — a scheme-instantiation var at a use site
  - `Idx(103)`: `Var(8)` state = `Unbound { id: 8, rank: Rank(3), name: None }` — another instantiation

- **Blast radius**: Affects all empty container literals (`[]`, `{}`, `Set<T>`) whose element types are inferred from downstream usage and where at least one use site doesn't fully constrain the element type. Because `.len()`, `.is_empty()`, and control-flow predicates on containers don't constrain element types, this is a wide class of real-world programs. Confirmed affected (tested):
  - `let ages = [];` + `ages.push(...)` + `ages.len()` — FAILS
  - `let $ages = [];` + `ages.push(...)` + `.len()` — FAILS (both mutable and immutable)
  - `let ages: [int] = []; ...` — WORKS (annotation monomorphizes)
  - `let ages = [0]; ages = ages.push(...)` — WORKS (non-empty has concrete element)

- **Affected files**:
  - `compiler/ori_types/src/infer/expr/blocks.rs:167` — `infer_let` must avoid generalization when it would leave unresolvable vars. The proposed change: guard generalization on the top-level tag of `init_ty`. Only generalize when the init type is a function (standard let-polymorphism for lambdas like `let id = x -> x`). Container types (`List`, `Option`, `Set`, `Map`, `Tuple`, `Range`, etc.) should NOT be generalized — local bindings of these types should remain monomorphic, with the fresh element var staying `Unbound` so downstream unification links it directly.

**Reference implementations**:
- **Rust `rustc_hir_typeck`**: Local `let` bindings are NEVER generalized. Rust has no let-polymorphism for local bindings — each binding must resolve to a single concrete type. `let x = vec![]; x.push(10);` works because inference flows forward monomorphically. This is the cleanest model for a compiler that monomorphizes for codegen.
- **OCaml, Elm, Gleam**: Generalize let bindings (HM), but these languages don't require monomorphic codegen (JS/bytecode/tree-walking). Their approach doesn't transfer to LLVM-targeting compilers.
- **Haskell GHC**: Has the monomorphism restriction specifically to avoid unresolved inference problems with polymorphic local bindings. Similar motivation to this fix.

---

## 1.5 Fix Consensus (via /tp-help)

Independent dual-source design review of the proposed fix approach. Ran BEFORE tests or implementation to catch wrong-approach errors before they lock in. See `.claude/skills/fix-bug/SKILL.md` § Phase 1.75 for the calling contract.

- **Proposed approach (pre-consensus)**: In `infer_let` at `compiler/ori_types/src/infer/expr/blocks.rs:167`, guard the `engine.generalize(init_ty)` call on the top-level tag of `init_ty`. Only generalize when `init_ty.tag() == Tag::Function | Tag::Scheme`. Bind the pattern to `init_ty` directly for other tags.
- **tp-help run scratch dir**: `/tmp/ori-tpr-VmdOpipn` (launched 2026-04-14 14:08:53 EDT, codex walltime 340s, gemini walltime 104s)

### Round 1

**Codex summary (LEAK + GAP + DRIFT findings)**:
- `LEAK` + `GAP`: **Proposed patch site is not the repro path.** Block-statement `let` inside a function body is handled by `infer_block` at `compiler/ori_types/src/infer/expr/blocks.rs:22-97`, generalizing at line 88 (and line 85 for non-capturing lambdas). `infer_let` at line 116 is only used for `ExprKind::Let` via `infer/expr/mod.rs:167-172`. A third duplicated policy exists in try-block lets at `sequences.rs:204-251` with generalize at line 247.
- `DRIFT`: **The spec already says the repro case is a compile-time error.** `docs/ori_lang/v2026/spec/14-expressions.md:1224-1228` states: "An empty list literal `[]` requires type context for inference. Without context, it is a compile-time error." Example: `let y = [];  // error: cannot infer element type`. The compiler is violating the spec by NOT emitting this error and silently passing to codegen.
- `GAP`: **PC-2/TR-2 is being enforced in the wrong phase.** Codegen detection (`ori_llvm/src/codegen/type_info/store.rs:341-363`) is too late. Typeck should emit `E2005` (ambiguous_type — already defined at `type_error/check_error/mod.rs:235-243` but no production call site) via a checker-exit sweep.
- Recommendation: fix ALL THREE generalize sites + wire `E2005` + add PC-2 validation sweep before exports/codegen.

**Gemini summary (DRIFT + GAP findings)**:
- `DRIFT`: **Tag-based check is brittle.** `matches!(tag, Function | Scheme)` fails when the resolved type is still a `Tag::Var` awaiting bi-directional unification (e.g., `let f = if cond then (x -> x) else (y -> y)`). Tag-based check would not generalize this even though it should. Use **AST-based Value Restriction**: check `ExprKind::Lambda` on the init's AST node instead.
- `GAP`: **(b) and (d) are not alternatives — they are two halves of the same phase contract.** MUST implement both: (d) skip generalization for containers (so `push` can unify directly), AND (b) emit `TypeCheckError("cannot infer type")` for any remaining `Tag::Var` at check exit. Otherwise, programs with no downstream constraint (`let xs = []; xs.len()`) leak `Unbound` vars to codegen.
- Spec verified: `docs/ori_lang/v2026/spec/13-variables.md` does NOT mandate let-polymorphism for local bindings. §13.6 requires value semantics ("Assignment is value copy") which is incompatible with polymorphic local bindings mutating types across calls. Standardizing on monomorphic local bindings (Rust-style) is fully spec-compliant.
- Recommendation: (1) Change generalization guard to AST-based Value Restriction (`ExprKind::Lambda`), (2) Implement final resolution pass emitting E2005, (3) Update `tests/spec/types/collections.ori` to annotate ambiguous empty lists.

**Agreement points (strong convergence)**:
1. My proposed fix is **incomplete and uses brittle tag-based detection**. Use AST-based Value Restriction instead.
2. **Phase contract enforcement MUST be added to the type checker** — a final-resolution sweep that emits `E2005` for any remaining `Tag::Var` in `expr_types`. Without this, PC-2/TR-2 is violated.
3. **Spec alignment is mandatory**: `14-expressions.md:1224-1228` already declares `let y = []` a compile-time error. The compiler is not enforcing its own spec.
4. There are **multiple generalization sites** (at least three: `infer_block` line 85/88, `infer_let` line 167, `sequences.rs:247`) — the fix must address all of them as an SSOT policy.
5. The existing code at `blocks.rs:79-89` ALREADY uses AST-based Lambda detection for capturing closures (same pattern needed for this fix) — this is precedent.

**Disagreement points**: None material. Both reviewers converge on the same recommendation.

**Independent code verification**:
- ✅ Verified `compiler/ori_types/src/infer/expr/blocks.rs:22-97`: block-statement let handler. Lines 79-89 show existing AST-based Lambda check for capturing closures. Generalization at line 85 (non-capturing lambdas) and line 88 (fallback — THIS is the repro path).
- ✅ Verified `docs/ori_lang/v2026/spec/14-expressions.md:1224-1228`: spec explicitly says `let y = []` is a compile-time error.
- ✅ Verified `compiler/ori_types/src/infer/expr/sequences.rs:204-251`: try-block let handler. Line 247 generalizes `bound_ty`.
- ✅ Verified `compiler/ori_types/src/infer/expr/blocks.rs:116-179` (`infer_let`): generalization at line 167. This IS a generalization site, but `infer_let` is only invoked by `ExprKind::Let` as a standalone expression, NOT by block-statement `let` — so my original proposed fix would have fixed a different path than the repro.
- ✅ Verified `E2005` (ambiguous_type) in `compiler/ori_types/src/type_error/check_error/`: the error code + constructor exist but have no production call site — this is a GAP per codex.

**Outcome**: Persuaded divergence — **I was wrong about both the patch site AND the completeness of the fix.** The revised approach integrates both reviewers' recommendations.

### Final agreed approach (revised after Plan TPR 2026-04-14)

**Multi-part point fix** (still scoped to `ori_types` crate, no plan escalation needed). REVISED scope after Plan TPR findings — see §R above:

1. **AST-based Value Restriction at all 3 generalization sites**. Extract a shared helper `should_generalize(arena: &ExprArena, init: ExprId) -> bool` that returns true ONLY if the init is a non-capturing `ExprKind::Lambda`. Apply at:
   - `compiler/ori_types/src/infer/expr/blocks.rs:79-89` — block-statement let in `infer_block` (primary repro path per TPR-04-004-codex)
   - `compiler/ori_types/src/infer/expr/blocks.rs:167` — `infer_let` (standalone let expression — rare surface, needs dedicated test per TPR-04-004-codex)
   - `compiler/ori_types/src/infer/expr/sequences.rs:247` — try-block let

   This SOLVES the primary repro: `ages.push(value: 10)` directly unifies the fresh Var(X) with `int` via `Link` (no generalization interposed), so `resolve_fully` resolves `List(Var(X))` → `List(int)` downstream.

2. **NARROWLY-SCOPED post-inference validation pass emitting E2005** (REVISED per TPR-04-001-codex, TPR-04-002-gemini, TPR-04-003-codex):

   **Do NOT walk every `expr_types` entry.** `infer_expr` stores subexpression types BEFORE `generalize` runs, and `is_polymorphic_lambda` in `ori_llvm/src/codegen/function_compiler/lambda_mono/type_resolve.rs:55-73` treats `Tag::Var` inside polymorphic lambdas as LEGITIMATE machinery. A blanket sweep would regress let-polymorphism.

   **Correct approach**: validate ONLY the declared types of let bindings (the BINDING's final type after generalization), not every sub-expression type. After `bind_pattern` is called with `final_ty`:
   - If `final_ty` contains any unresolved `Tag::Var` that is NOT bound by an enclosing `Tag::Scheme`, emit E2005 at the binding's span.
   - Use `bound_vars: &FxHashSet<u32>` (collected from each enclosing `pool.scheme_vars()`) to distinguish scheme-bound vars from unresolved vars. Boolean `under_scheme` is wrong — schemes bind specific var_ids only.
   - **Cascade suppression**: skip E2005 emission when `engine.has_errors()` is true OR when `final_ty` already contains `Tag::Error` / `TypeFlags::HAS_ERROR`. Prevents UN-4 monotonicity violation.

   This narrows validation to the specific surface the spec (14-expressions.md:1224-1228) governs: local `let` bindings whose type is ambiguous at declaration time.

3. **Scope NARROWED to empty lists only** (per TPR-04-006-codex):
   - Spec at 14-expressions.md:1224-1228 only declares `[]` a compile-time error.
   - Empty maps `{}` are spec-neutral (only mentioned as syntax parsing).
   - `Set<int>()` is NOT valid Ori syntax; `infer_empty_set()` does not exist.
   - The validation pass applies to ANY list type that remains ambiguous at binding — no special-casing by constructor. But tests only cover the list case (§2 TDD Matrix revised accordingly).

4. **Recursive negative pin** (per TPR-04-002-codex): use `contains_var` (modeled after `ori_llvm/src/codegen/function_compiler/lambda_mono/type_predicates.rs:10-25`) to walk `expr_types` recursively and assert no unresolved Vars at any depth for the repro program.

5. **Test updates broadened** (per TPR-04-005-codex): audit includes `[].iter()`, `[].iter().rev().collect()`, `[].iter().last()`, `[].iter().rfind(...)`, `[].iter().rfold(...)` in `double_ended_methods.ori` lines 35, 84, 133, 174 — not just `let name = []` bindings.

6. **Algorithmic DRY follow-up**: file separately as BUG-04-{next} per TPR-04-003-gemini. Consolidation of the three let-binding sites into `bind_local_let` is out of scope for this fix.

7. **Positive tests**:
   - Repro: `let ages = []; ages = ages.push(value: 10); if ages.len() == 1 then 0 else 1` compiles via AOT.
   - Lambda let-polymorphism: `let id = x -> x; id(1); id("hello")` MUST still work (new semantic pin per TPR-04-001-codex).
   - Capturing lambda: unchanged existing behavior.

8. **Negative pins**:
   - `let xs = []; xs.len()` — E2005 at check time (not codegen).
   - `let x = [] ; unknown_fn(x)` — only `UnknownIdent` error fires, NOT spurious E2005 (cascade suppression pin per TPR-04-003-codex).
   - Regression pin: recursive `contains_var` walk of repro's typed IR shows no unresolved Vars.

---

## 2. TDD — Test Matrix (revised 2026-04-14 after Plan TPR)

Write ALL tests BEFORE the fix. Verify they fail against current code.

### Exact failing case
- [ ] `test_aot_empty_list_with_push_inferred_element_compiles` — the exact repro from the bug entry: `@main () -> int = { let ages = []; ages = ages.push(value: 10); if ages.len() == 1 then 0 else 1 }` compiles via `ori build` and runs exit 0.

### Edge cases — multiple container types via block-statement let (the repro path per codex finding)
- [ ] `test_empty_list_push_multiple_times_resolves_to_int_via_block_let` — `let xs = []; xs = xs.push(value: 1); xs = xs.push(value: 2); xs.len() == 2`
- [ ] `test_empty_list_with_annotation_compiles_unchanged` — `let xs: [int] = []; xs = xs.push(value: 10)` — regression guard
- [ ] `test_empty_list_inferred_from_first_push_resolves_element` — `let xs = []; xs = xs.push(value: 42)` — push constrains element

### Cross-type coverage (block-statement let — repro path)
- [ ] `test_empty_list_block_let_element_int_via_push` — `let xs = []; xs = xs.push(value: 10)`
- [ ] `test_empty_list_block_let_element_str_via_push` — `let xs = []; xs = xs.push(value: "hello")`
- [ ] `test_empty_list_block_let_element_bool_via_push` — `let xs = []; xs = xs.push(value: true)`

### Cross-generalization-site coverage (CRITICAL per TPR-04-004-codex — 3 distinct paths, revised)
- [ ] `test_empty_list_standalone_let_expr_routes_through_infer_let` — Rust unit test in `ori_types/src/infer/expr/tests.rs` that constructs a standalone `ExprKind::Let` AST node (via parser surface `@test () -> void = let x = 1;` confirmed in `ori_parse/src/tests/parser.rs:130-167, 1703-1730`) and asserts the dispatch goes through `infer_let`, NOT `infer_block`. The prior plan's wording "let xs = [] in xs.push(value: 10)" does NOT route through `infer_let` — block-statement let routes through `infer_block`. This test MUST force the actual `infer_let` dispatch.
- [ ] `test_empty_list_try_block_let_compiles` — covers `sequences.rs:247` path — inside a `try { let xs = ...; ... }` block

### Lambda let-polymorphism preservation (must still work after fix)
- [ ] `test_lambda_let_polymorphism_identity_used_at_multiple_types` — `let id = x -> x; let a = id(1); let b = id("hello"); ...` — must continue to work
- [ ] `test_lambda_let_polymorphism_pair` — `let pair = x -> y -> (x, y); pair(1)(true); pair("a")(1.0)` — generalize should fire for lambda
- [ ] `test_capturing_lambda_no_generalize_regression` — non-capturing lambda generalizes, capturing lambda does not (existing behavior at `blocks.rs:79-89` preserved)

### Ambiguous type error emission (revised per TPR-04-006-codex: scope narrowed to lists only)
- [ ] `test_empty_list_ambiguous_no_constraint_emits_E2005` — `let xs = []; xs.len()` (no element constraint) MUST emit `E2005` at type check time (NOT codegen). Negative pin: `#compile_fail("cannot infer")` matches E2005 message.
- [ ] `test_truly_polymorphic_untyped_rejected_at_check_not_codegen` — key semantic pin: the error surfaces in `ori check` (typeck phase), not only in `ori build` (codegen phase).

**Removed** (per TPR-04-006-codex — spec doesn't mandate, implementation doesn't exist):
- ~~`test_empty_map_ambiguous_emits_E2005`~~ — spec at 14-expressions.md:1238-1240 is neutral on `let m = {}`; out of scope for this fix.
- ~~`test_empty_set_ambiguous_emits_E2005`~~ — `Set<int>()` is NOT valid Ori syntax; `infer_empty_set()` does not exist.

### Cascade suppression (NEW — per TPR-04-003-codex + TPR-04-001-gemini)
- [ ] `test_empty_list_inside_already_error_typed_expression_suppresses_E2005` — program: `let x = []; fop(x)` where `fop` is undefined. Should emit ONLY `UnknownIdent` for `fop`, NOT a cascading E2005 for the empty list `x`. Verifies typeck.md UN-4 recovery monotonicity is preserved.
- [ ] `test_empty_list_in_malformed_call_suppresses_E2005` — program with parse-level or type-level error surrounding the empty list, where the primary error is sufficient — validates cascade suppression at multiple error shapes.

### Semantic pins (only pass with the correct new behavior)
- [ ] `test_generalize_skipped_for_list_literal_init` — Rust unit test in `ori_types`: construct an `infer_block` scenario with `StmtKind::Let { init: List(empty) }`, assert that the stored `expr_types[init_id]` resolves via `pool.resolve_fully()` to `List(int)` after a subsequent `push(value: 10)` unifies the element. If generalization were still firing, the var would be `Generalized` and resolution would fail.
- [ ] `test_generalize_fires_for_lambda_literal_init` — Rust unit test: same scenario but with `StmtKind::Let { init: Lambda { ... } }`, assert that generalization DOES fire (scheme is produced).

### Negative pins (reject the old broken behavior — revised per TPR-04-002-codex)
- [ ] `test_no_unresolved_var_in_repro_expr_types_recursive` — Rust integration test using a `contains_var` helper (modeled after `ori_llvm/src/codegen/function_compiler/lambda_mono/type_predicates.rs:10-25`) that walks `expr_types` RECURSIVELY. For the repro program, assert NO `expr_type` Idx contains `Tag::Var` at any depth (top-level OR nested inside container types). Shallow check is insufficient — the bug shape is `List(Var(X))` where top is `List`. This test MUST fail against current code.
- [ ] `test_ambiguous_empty_list_rejected_at_check_not_codegen` — see above; pins that the spec-mandated error surfaces in typeck, not codegen.

### Lambda polymorphism regression pins (NEW — per TPR-04-001-codex)
- [ ] `test_polymorphic_lambda_with_container_still_type_checks` — sanity pin: `let id = x -> x; let a = id([1, 2]); let b = id([])` — verifies that after the fix, lambda polymorphism is NOT affected by container-binding monomorphization. Schema-bound vars inside lambda body type must remain exempt from E2005.
- [ ] `test_polymorphic_lambda_captures_outer_ambiguous_empty_rejected` — important edge case: `{ let empty = []; let f = () -> empty; f() }` — the outer `empty` is ambiguous (len never constrains). After fix, E2005 on `empty`, because `empty` is not a lambda binding. The closure `f` capturing `empty` doesn't change `empty`'s ambiguity.

### Cross-phase parity
- [ ] `test_empty_list_push_interpreter_and_llvm_parity` — the repro runs identically under `ori run` (interpreter) and `ori build + exec` (AOT). Dual-execution check per `canon.md §4.5`.

### Verify tests fail before fix
- [ ] All new AOT tests fail against current code (confirming they test the bug)
- [ ] All new typeck tests fail against current code OR pass trivially (document which)

---

## 2.5 Fix Plan TPR Findings

**Gate:** Mandatory — severity `high` AND complexity-elevated subsystems (`ori_types` — type inference, `ori_llvm` — codegen).

*To be filled after Plan TPR runs in Phase 2.5.*

---

## 3. Implementation

Four coordinated components per consensus — all in `ori_types` crate:

### 3.1 Extract `should_generalize` helper (SSOT policy)

Add a shared policy function in `compiler/ori_types/src/infer/expr/blocks.rs` (or a new `generalization_policy.rs` module if cleaner):

```rust
/// Determines whether a let-binding initializer should be generalized
/// for let-polymorphism. Uses AST-based Value Restriction: only lambdas
/// are generalized; all other initializers are bound monomorphically.
///
/// This is a critical phase-contract enforcement: generalizing non-lambda
/// initializers (e.g., empty collection literals) leaves `Tag::Var` with
/// `VarState::Generalized` in `expr_types`, which violates typeck PC-2
/// (no Tag::Var in typed IR output) and causes codegen failures per
/// codegen-rules.md TR-2.
///
/// Non-capturing lambdas are generalized (preserves `let id = x -> x`
/// polymorphism). Capturing lambdas are NOT generalized (existing
/// behavior at `blocks.rs:79-89` — capturing closures cannot be
/// monomorphized by AOT codegen).
///
/// Fix: BUG-04-074 (plans/bug-tracker/fix-BUG-04-074.md)
/// Spec: docs/ori_lang/v2026/spec/14-expressions.md §14.17.1 (empty lists require type context)
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

### 3.2 Apply the helper at all 3 generalization sites

**Site A — `infer_block` block-statement let** at `compiler/ori_types/src/infer/expr/blocks.rs:79-89`:

```rust
// BEFORE (current code):
if let ExprKind::Lambda { params, body, .. } = &arena.get_expr(*init).kind {
    let param_names: Vec<Name> =
        arena.get_params(*params).iter().map(|p| p.name).collect();
    if body_captures_outer(arena, *body, &param_names) {
        init_ty
    } else {
        engine.generalize(init_ty)
    }
} else {
    engine.generalize(init_ty)  // ← BUG: generalizes non-lambda initializers
}

// AFTER:
if should_generalize(arena, *init) {
    engine.generalize(init_ty)
} else {
    init_ty
}
```

**Site B — `infer_let` standalone let expression** at `compiler/ori_types/src/infer/expr/blocks.rs:116-168`:

```rust
// BEFORE:
engine.generalize(init_ty)

// AFTER:
if should_generalize(arena, init) {
    engine.generalize(init_ty)
} else {
    init_ty
}
```

Note: `infer_let` already has capture-detection at lines 159-163; the new helper subsumes that check.

**Site C — `sequences.rs` try-block let** at `compiler/ori_types/src/infer/expr/sequences.rs:243-247`:

```rust
// BEFORE:
let bound_ty = unwrap_result_or_option(engine, init_ty);
engine.generalize(bound_ty)

// AFTER:
let bound_ty = unwrap_result_or_option(engine, init_ty);
if should_generalize(arena, *init) {
    engine.generalize(bound_ty)
} else {
    bound_ty
}
```

### 3.3 Narrowly-scoped post-inference validation pass (E2005 emission — REVISED 2026-04-14)

**CRITICAL revision after Plan TPR**: do NOT walk ALL `expr_types`. That would regress let-polymorphism (TPR-04-001-codex) because `infer_expr` stores lambda types with `Function(Var, Var)` BEFORE `generalize` runs, and `is_polymorphic_lambda` at `ori_llvm/codegen/function_compiler/lambda_mono/type_resolve.rs:55-73` treats those Vars as legitimate machinery.

**Correct approach**: validate ONLY the declared types of let bindings (the BINDING's final type after generalization), applied IMMEDIATELY AFTER `bind_pattern` is called with `final_ty`. This is the narrow surface the spec governs.

Add validation AT THE LET-BINDING SITES (not a separate post-body sweep):

```rust
// In blocks.rs infer_block, after the existing `bind_pattern(engine, arena, pat, final_ty)`:

validate_binding_type_resolved(engine, arena, pat, final_ty, stmt.span);

// Similar insertion in infer_let (blocks.rs:116) and the try-block let (sequences.rs).
```

The validation helper:

```rust
/// Emit E2005 if a let-binding's declared type contains unresolved vars
/// that are not legitimately bound by an enclosing Scheme.
///
/// Cascade suppression: skip when Tag::Error is present (HAS_ERROR flag)
/// or when the engine has existing errors — per typeck.md UN-4 recovery
/// monotonicity (TPR-04-003-codex + TPR-04-001-gemini).
///
/// Scheme boundary: uses a FxHashSet<u32> of bound var IDs (from
/// pool.scheme_vars()), NOT a boolean — schemes bind specific var_ids
/// only (TPR-04-002-gemini).
///
/// Fix: BUG-04-074
fn validate_binding_type_resolved(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    pat: &BindingPattern,
    final_ty: Idx,
    span: Span,
) {
    // Cascade suppression: don't emit secondary errors when the
    // engine is already in an error state or when the type itself
    // is poisoned.
    if engine.has_errors() {
        return;
    }
    if engine.pool().flags(final_ty).contains(TypeFlags::HAS_ERROR) {
        return;
    }

    // Walk final_ty with a bound-vars set. Start with empty set — any
    // Tag::Scheme encountered adds its scheme_vars to the set before
    // recursing into its body.
    let mut bound = FxHashSet::default();
    if has_unbound_var(engine.pool(), final_ty, &mut bound) {
        engine.record_error(TypeCheckError::ambiguous_type(span));
        // E2005 — "cannot infer type; add a type annotation"
    }
}

/// Recursively check if `ty` contains a Tag::Var whose var_id is NOT in `bound`.
/// When a Tag::Scheme is encountered, its scheme_vars are added to `bound`
/// for the recursion into its body.
fn has_unbound_var(pool: &Pool, ty: Idx, bound: &mut FxHashSet<u32>) -> bool {
    // Fast path: no vars present
    if !pool.flags(ty).contains(TypeFlags::HAS_VAR) {
        return false;
    }

    match pool.tag(ty) {
        Tag::Var => {
            let var_id = pool.data(ty);
            // Unbound OR Generalized not in `bound` → ambiguous
            !bound.contains(&var_id)
        }
        Tag::Scheme => {
            // Add scheme's bound vars to set, recurse into body, remove them
            let scheme_vars = pool.scheme_vars(ty).to_vec();
            let scheme_body = pool.scheme_body(ty);
            for v in &scheme_vars {
                bound.insert(*v);
            }
            let result = has_unbound_var(pool, scheme_body, bound);
            for v in &scheme_vars {
                bound.remove(v);
            }
            result
        }
        // Recurse into children for container types
        Tag::List | Tag::Option | Tag::Set | Tag::Range | Tag::Iterator | Tag::DoubleEndedIterator => {
            has_unbound_var(pool, Idx::from_raw(pool.data(ty)), bound)
        }
        Tag::Map | Tag::Result => {
            // two-child
            let extra_off = pool.data(ty) as usize;
            let a = Idx::from_raw(pool.extra(extra_off));
            let b = Idx::from_raw(pool.extra(extra_off + 1));
            has_unbound_var(pool, a, bound) || has_unbound_var(pool, b, bound)
        }
        Tag::Function | Tag::Tuple | Tag::Applied | Tag::Struct | Tag::Enum => {
            // Walk all child types via pool accessors
            pool.children(ty).any(|child| has_unbound_var(pool, child, bound))
        }
        _ => false,  // primitives, Error, etc.
    }
}
```

**Key differences from v1 plan**:
- Runs AT EACH LET BINDING, not on all `expr_types`
- `final_ty` is the BINDING's type (post-generalization), not every sub-expression
- Uses `FxHashSet<u32>` for bound-var tracking, not `bool`
- Cascade suppression via `engine.has_errors()` + `HAS_ERROR` flag check
- Recursive walk with scheme-bound-vars stack (push/pop on Scheme)

**Helper module location** (per TPR-04-007-codex + helper-visibility question):
- Place `should_generalize` (§3.1) + `validate_binding_type_resolved` + `has_unbound_var` in a new module `compiler/ori_types/src/infer/expr/generalization_policy.rs`.
- Use `pub(crate)` or `pub(super)` visibility so `infer_block`, `infer_let`, and `sequences.rs` can all call them cleanly across sibling module boundaries.

Wire up the `ambiguous_type` constructor at `type_error/check_error/mod.rs:235-243` as the error emission point. The current message "cannot infer type" is adequate for the list case (scope narrowed per TPR-04-006-codex; tailored messages per TPR-04-004-gemini become moot).

### 3.4 Test updates (broadened per TPR-04-005-codex)

- [ ] Audit `tests/spec/types/collections.ori` — add type annotations to any active `let empty = []` bindings. Most are already commented out per codex's investigation.
- [ ] Audit `tests/spec/traits/iterator/double_ended.ori:25-34, 66-82` — dead-local `let result = []` sites; annotate if they fail E2005.
- [ ] Audit `tests/spec/traits/iterator/double_ended.ori:167` — direct-receiver `[].iter()` form.
- [ ] Audit `tests/spec/traits/iterator/double_ended_methods.ori:35, 84, 133, 174` — direct-receiver forms: `[].iter().rev().collect()`, `[].iter().last()`, `[].iter().rfind(...)`, `[].iter().rfold(...)`.
- [ ] Full repo sweep: `rg -n 'let \w+ = \[\];|let \$?\w+ = \[\];|\[\]\.iter\(\)|\[\]\.len\(\)|\[\]\.is_empty\(\)' tests/ library/` to discover any remaining direct-receiver or binding-site forms NOT yet listed.
- [ ] For each affected test file, decide: annotate (`let xs: [int] = []`) OR remove dead local OR mark `#compile_fail` if the test's purpose is to verify error emission.

### 3.5 Algorithmic DRY follow-up (out of scope — per TPR-04-003-gemini)

The three let-binding sites (`infer_block`, `infer_let`, `sequences.rs`) have duplicated surrounding logic (capture detection, branch on generalize, bind pattern). Consolidating into a single `bind_local_let` abstraction is a valid `LEAK:algorithmic-duplication` concern per `impl-hygiene.md` §Algorithmic DRY but is OUT OF SCOPE for this bug fix.

**Concrete tracking artifact** (required per CLAUDE.md "future improvement" rule — must not be a nebulous deferral):
- [ ] Action: file `BUG-04-{next}` via `/add-bug` at close-out of BUG-04-074 with title "Consolidate let-binding inference across infer_block/infer_let/sequences.rs into shared helper" and subsystem `ori_types`. Severity: `low` (code hygiene, not correctness).

### 3.5 Implementation order

1. Write all tests from §2 (including negative pins that REQUIRE E2005 emission)
2. Verify tests fail appropriately (some fail on codegen error, some pass trivially — document which)
3. Implement §3.1 helper + §3.2 three-site replacement → verifies the main repro compiles
4. Implement §3.3 validation sweep → verifies ambiguous cases produce E2005 at check
5. Apply §3.4 test updates as needed
6. Run `timeout 150 ./test-all.sh` — full suite green
7. Run `timeout 150 ./clippy-all.sh` — no warnings
8. Run `/commit-push`, then Phase 5 (TPR + hygiene)

---

## R. Third Party Review Findings

### Phase 2.5 — Plan TPR (2026-04-14)

Plan TPR run `/tmp/ori-tpr-DZNWHvXU`. codex walltime 436s, gemini walltime 232s, ASYMMETRY LOW (comparable depth). 12 findings (10 actionable + 2 informational). Adversarial review flagged multiple critical flaws in the plan's validation-pass design.

- [x] `[TPR-04-001-codex][high]` `plans/bug-tracker/fix-BUG-04-074.md:283` — Narrow the E2005 sweep so valid lambda polymorphism survives.
  Resolved: Revised §3.3 on 2026-04-14. Validation pass is now NARROWLY SCOPED to let-binding declared types (final_ty after generalization), NOT every expr_types entry. Added `test_polymorphic_lambda_with_container_still_type_checks` positive pin. Sub-expression Vars from pre-generalization storage are no longer touched, preserving lambda_mono/type_resolve.rs:55-73's legitimate Var usage.
  Evidence: `infer_expr` stores every subexpression type before the caller generalizes (`ori_types/src/infer/expr/mod.rs:272`), so `let id = x -> x` leaves the lambda typed as `Function(Var, Var)` in expr_types. Downstream `is_polymorphic_lambda` at `ori_llvm/src/codegen/function_compiler/lambda_mono/type_resolve.rs:55-73` EXPLICITLY treats generalized `Tag::Var` inside polymorphic lambdas as legitimate machinery, using `contains_var` for deep checks. My blanket sweep would turn BUG-04-074 into a let-polymorphism regression.
  Required plan update: Validate only ambiguity surfaces that must be concrete at body exit, not every `expr_types` entry. Either (a) rewrite generalized let-initializer `expr_types` to `Scheme` before storage so the walk sees bound-var-correct shapes, or (b) make the sweep AST/context-aware so legitimate lambda-polymorphism surfaces are exempt. Add an explicit positive pin proving `let id = x -> x; id(1); id("hello")` still passes.
  Basis: direct_file_inspection. Confidence: high. (Verified independently: `type_resolve.rs:47-73` confirmed.)

- [x] `[TPR-04-002-gemini][high]` `plans/bug-tracker/fix-BUG-04-074.md:237` — Track bound variable IDs in `has_unresolved_var` instead of a boolean.
  Resolved: Revised §3.3 on 2026-04-14. `has_unbound_var` now uses `FxHashSet<u32>` for bound var IDs. When entering a Scheme, scheme_vars are pushed onto the set; popped on exit. A `Tag::Var` is "unbound" only if its var_id is NOT in the current set. This correctly handles captured vars inside closure lambdas — they aren't in the scheme's bound set, so they're flagged as ambiguous.
  Evidence: A scheme only binds SPECIFIC `var_ids` (those returned by `pool.scheme_vars()` — verified at `ori_types/src/unify/generalization.rs:47-58` where `pool.scheme(&vars, ty)` binds exactly those). If a closure captures an outer empty collection, the outer `Tag::Var` inside the scheme's body is NOT bound by that scheme. The proposed `walk_type(pool, ty, /*under_scheme=*/ false)` with a boolean flag would wrongly exempt such captured Vars.
  Required plan update: Update `has_unresolved_var` to track the exact set of bound variable IDs (e.g., passing down a `&FxHashSet<u32>`) and only exempt `Var`s whose IDs are present in the bound set, not all Vars under any Scheme.
  Basis: fresh_verification. Confidence: high. (Verified independently: `generalization.rs:47-58` confirmed.)

- [x] `[TPR-04-003-codex][high]` `plans/bug-tracker/fix-BUG-04-074.md:283` — Suppress E2005 on error-poisoned subexpressions.
  Resolved: Revised §3.3 on 2026-04-14. Added cascade-suppression guard at top of `validate_binding_type_resolved`: skip when `engine.has_errors()` is true OR when `final_ty` has `TypeFlags::HAS_ERROR` set. Added `test_empty_list_inside_already_error_typed_expression_suppresses_E2005` negative pin.
  Evidence: GAP against typeck.md UN-4 and impl-hygiene.md §Error Recovery Monotonicity. `infer_expr` stores every subexpression type eagerly (`ori_types/src/infer/expr/mod.rs:272`), while failing paths return `Idx::ERROR` only at outer expressions. An empty literal nested inside a broken expression still sits in `expr_types` as `List(Var)` and would pick up a second E2005 even though the primary diagnostic explains the failure.
  Required plan update: Specify suppression rules in §3.3: skip any `expr_type` whose tree contains `Tag::Error` / `HAS_ERROR`. Add a negative recovery test where an empty literal inside an already-error-typed expression does NOT emit E2005.
  Basis: direct_file_inspection. Confidence: high.

- [x] `[TPR-04-001-gemini][high]` `plans/bug-tracker/fix-BUG-04-074.md:214` — Suppress E2005 emission if engine already has errors.
  Resolved: Same fix as [TPR-04-003-codex] — see above. `engine.has_errors()` guard + `TypeFlags::HAS_ERROR` check at top of validation helper.
  Evidence: Same class as TPR-04-003-codex. If `let x = []` with a downstream typo (`fop(x)` instead of `foo(x)`), the typechecker emits `UnknownIdent` and `x` remains unresolved. Indiscriminate E2005 emission causes cascading errors, violating UN-4 recovery monotonicity.
  Required plan update: Add `engine.has_errors()` guard or `Tag::Error` unification check before emitting E2005.
  Basis: fresh_verification. Confidence: high. (Cross-reference: TPR-04-003-codex — agreement on semantics even though merger didn't auto-detect.)

- [x] `[TPR-04-002-codex][medium]` `plans/bug-tracker/fix-BUG-04-074.md:164` — Make the no-Var regression pin recursive.
  Resolved: Revised §2 TDD Matrix on 2026-04-14. Negative pin `test_no_unresolved_var_in_repro_expr_types_recursive` now uses recursive `contains_var` helper modeled after `lambda_mono/type_predicates.rs:10-25` to check Var at any depth.
  Evidence: The proposed negative pin iterates `TypedModule.expr_types` and checks top-level `Tag::Var`. The bug shape is `List(Var(X))` where the TOP is `List` but the CHILD is `Var`. Plan's shallow check would pass while the bug remains. Repo already has `contains_var` at `ori_llvm/src/codegen/function_compiler/lambda_mono/type_predicates.rs:10-25`.
  Required plan update: Change negative pin to walk each expr_type recursively with `contains_var`-style helper. Assert the specific repro's empty-list expr_type has no unresolved Vars at any depth.
  Basis: direct_file_inspection. Confidence: high.

- [x] `[TPR-04-004-codex][medium]` `plans/bug-tracker/fix-BUG-04-074.md:146` — Hit the actual standalone let-expression path.
  Resolved: Revised §2 TDD Matrix on 2026-04-14. Replaced test case with Rust unit test `test_empty_list_standalone_let_expr_routes_through_infer_let` that constructs `ExprKind::Let` directly and asserts dispatch through `infer_let`, not `infer_block`. Test uses the parser surface `@test () -> void = let x = 1;` confirmed at `ori_parse/src/tests/parser.rs:130-167`.
  Evidence: `infer/expr/mod.rs:159-173` routes `ExprKind::Block` through `infer_block` and `ExprKind::Let` through `infer_let`. Block-statement `let` inside `@main () -> int = { let x = ...; x }` dispatches to `infer_block`, NOT `infer_let`. Standalone let expression is `@test () -> void = let x = 1;` (parser surface at `ori_parse/src/tests/parser.rs:130-167, 1703-1730`). Plan's infer_let coverage cell doesn't force the intended dispatch.
  Required plan update: Replace the `infer_let` coverage cell with a real standalone `ExprKind::Let` program OR a focused Rust unit test that necessarily dispatches through `infer_let`.
  Basis: direct_file_inspection. Confidence: high. (Verified independently: `infer/expr/mod.rs:159-173` confirmed.)

- [x] `[TPR-04-005-codex][medium]` `plans/bug-tracker/fix-BUG-04-074.md:330` — Expand the test audit beyond two dead locals.
  Resolved: Revised §3.4 on 2026-04-14. Test audit now includes direct-receiver `[].iter()` forms: `double_ended.ori:167` and `double_ended_methods.ori:35, 84, 133, 174`. Added full repo sweep step: `rg -n 'let \w+ = \[\];|let \$?\w+ = \[\];|\[\]\.iter\(\)|\[\]\.len\(\)|\[\]\.is_empty\(\)' tests/ library/`.
  Evidence: §3.4 only cites `double_ended.ori:25-34, 66-82`, but active suite coverage also uses uncontextualized empty-list receivers: `[].iter()` at `double_ended.ori:167`; `[].iter().rev().collect()`, `[].iter().last()`, `[].iter().rfind(...)`, `[].iter().rfold(...)` at `double_ended_methods.ori:35, 84, 133, 174`. Plan misses direct-receiver forms entirely.
  Required plan update: Broaden §3.4 to audit ALL uncontextualized empty-literal forms — direct receivers (`[].iter()`) not just `let name = []` bindings. Name all currently-discovered active files and update them.
  Basis: direct_file_inspection. Confidence: high.

- [x] `[TPR-04-006-codex][medium]` `plans/bug-tracker/fix-BUG-04-074.md:155` — Narrow or re-spec the map and set portion of the fix.
  Resolved: Revised §1.5, §2, §3 on 2026-04-14. Scope NARROWED to empty lists only — dropped `{}` and `Set<int>()` test cases. Rationale: spec at `14-expressions.md:1224-1228` only declares `[]` a compile-time error; `{}` parsing is spec-neutral (1238-1240); `Set<int>()` is NOT valid Ori syntax and `infer_empty_set()` does NOT exist (verified). The validation pass WILL catch ambiguous maps `let m = {}; m.len()` if they arise (it's type-agnostic — walks any Tag::Var), but tests focus on the spec-sanctioned list case.
  Evidence: Spec at `docs/ori_lang/v2026/spec/14-expressions.md:1224-1228` only declares `[]` without context a compile-time error. For `{}`, spec at 1238-1240 only says it parses as empty map literal — no normative rejection of `let m = {}`. For sets, documented empty construction is `Set.new()` or `[].iter().collect()` (not `Set<int>()` which is NOT valid Ori syntax). `infer_empty_set()` does not exist as a function (verified).
  Required plan update: Either (a) narrow the fix scope and success criteria to empty-list ambiguity only, or (b) add an explicit spec investigation step for empty maps + real empty-set construction path BEFORE including them in goals, diagnostics, or tests. If (b), may require a spec proposal.
  Basis: direct_file_inspection. Confidence: high. (Verified independently: `infer_empty_map` DOES exist at `infer/mod.rs:630`; `infer_empty_set` does NOT exist.)

- [x] `[TPR-04-003-gemini][medium]` `plans/bug-tracker/fix-BUG-04-074.md:154` — Consolidate `infer_let` control flow to fix Algorithmic Duplication.
  Resolved: Added §3.5 with concrete tracking artifact on 2026-04-14. The consolidation is out-of-scope for BUG-04-074 per CLAUDE.md narrow-the-front discipline, but tracked for close-out: file `BUG-04-{next}` via `/add-bug` at close with title "Consolidate let-binding inference across infer_block/infer_let/sequences.rs into shared helper", subsystem `ori_types`, severity `low`.
  Evidence: Per `impl-hygiene.md` §Algorithmic DRY ("same fix at 3+ callsites = missing abstraction"), my plan extracts `should_generalize` but leaves the surrounding multi-step let-binding algorithm (detect capture, branch on generalize, bind pattern) duplicated across `infer_block`, `infer_let`, and `sequences.rs`.
  Required plan update: Add a step to consolidate the let-binding logic into a single `bind_local_let` abstraction — OR explicitly mark it as out-of-scope for this fix with a concrete tracking artifact (bug or roadmap item).
  Basis: direct_file_inspection. Confidence: medium.

- [x] `[TPR-04-004-gemini][medium]` `plans/bug-tracker/fix-BUG-04-074.md:228` — Tailor E2005 suggestion message dynamically.
  Resolved: Moot after scope narrowing per [TPR-04-006-codex]. Since tests only cover empty lists, the existing E2005 message "cannot infer type" is adequate. If future work extends to maps/sets (via a spec proposal), message tailoring should be added then. No action needed for this fix.
  Evidence: Plan proposes E2005 message "add a type annotation like `let x: [int] = []`". But if the sweep covers `{}` and sets (per TPR-04-006-codex this may be dropped), a hardcoded list suggestion is misleading for maps. Violates impl-hygiene.md §Diagnostic Message Quality ("show the fix").
  Required plan update: If empty map/set stay in scope, validation pass should inspect `arena.get_expr(expr_idx).kind` to tailor: `let x: {str: int} = {}` for maps, `let x: [int] = []` for lists. If scope narrows to lists only (per TPR-04-006-codex), this is moot.
  Basis: direct_file_inspection. Confidence: high.

- [x] `[TPR-04-007-codex][informational]` `plans/bug-tracker/fix-BUG-04-074.md:189` — Helper extraction and per-body hook are structurally sound.
  Evidence: Confirmed 3 current let-generalization sites (`infer_block`, `infer_let`, try-block `let`). CK-1 body-pass structure supports body-local validation. Centralizing the AST-based value-restriction predicate is the right direction.
  Resolved: Confirmed — keeping the helper centralized and the per-body hook as planned.

- [x] `[TPR-04-005-gemini][informational]` `plans/bug-tracker/fix-BUG-04-074.md:243` — Downstream `ori fmt` and `ori run` are safe.
  Evidence: `ori fmt` uses parser only — no typechecking dependency. `ori run` blocks evaluation when `has_errors()` is true. Plan's downstream assessments are accurate.
  Resolved: Confirmed — no plan updates needed for fmt/run paths.

### Plan revisions applied

Based on the verified findings, §1.5 Fix Consensus, §2 TDD Matrix, and §3 Implementation will be revised before Phase 3 (TDD writing). Key revisions:

1. **Narrow the validation pass target** (addresses TPR-04-001-codex + TPR-04-002-gemini): instead of walking ALL `expr_types`, validate only at specific exit points OR rewrite generalized let-init expr_types to Scheme shape before storage.
2. **Scoped bound-var tracking** (addresses TPR-04-002-gemini): replace `under_scheme: bool` with `bound_vars: &FxHashSet<u32>` passed through the walk.
3. **Cascade suppression** (addresses TPR-04-003-codex + TPR-04-001-gemini): skip E2005 emission when `HAS_ERROR` flag is set on the type or when `engine.has_errors()`.
4. **Narrow scope to lists only** (addresses TPR-04-006-codex): drop `{}` and `Set<int>()` from the test matrix; spec only mandates `[]` rejection. Remove `infer_empty_set()` references (no such function exists).
5. **Recursive negative pin** (addresses TPR-04-002-codex): use `contains_var` helper for deep checking.
6. **Real `infer_let` dispatch test** (addresses TPR-04-004-codex): add a program that actually routes through `infer_let` via standalone `let x = 1;` syntax.
7. **Broader test audit** (addresses TPR-04-005-codex): audit `[].iter()` and similar direct-receiver forms, not just `let name = []`.
8. **Algorithmic DRY follow-up** (addresses TPR-04-003-gemini): file as follow-up bug (BUG-04-{next}) rather than in-scope refactor — keep this fix narrowly focused.

---

## 4. Completion Checklist

Reviews MUST complete before bug closure.

- [ ] All new tests pass unchanged after fix (no test modifications needed)
- [ ] Matrix completeness verified — every cell in type x pattern x feature grid has a test
- [ ] Debug AND release builds pass (`cargo b && cargo b --release`)
- [ ] Interpreter and LLVM produce identical results for all new tests (dual-execution parity)
- [ ] `timeout 150 ./test-all.sh` green — no regressions
- [ ] `timeout 150 ./clippy-all.sh` green
- [ ] `cargo test -p ori_types` green
- [ ] `cargo test -p ori_llvm` green
- [ ] `/commit-push` — commit all changes before review
- [ ] Plan TPR (Phase 2.5) — completed (mandatory)
- [ ] `/tpr-review` (Phase 5 — code review) passed
- [ ] `/impl-hygiene-review` passed
- [ ] `/improve-tooling` retrospective completed
- [ ] `/sync-claude` doc sync verified
- [ ] Bug entry in `plans/bug-tracker/section-04-codegen-llvm.md` updated: `- [x]` with resolution details
- [ ] Fix section frontmatter `status` updated to `complete`
- [ ] Final `/commit-push`

**Exit Criteria:** The command `timeout 30 cargo run -q -- build /tmp/bug_04_074_repro.ori -o /tmp/test` exits 0 with no codegen errors when the repro file contains `let ages = []; ages = ages.push(value: 10); if ages.len() == 1 then 0 else 1`. The produced binary `/tmp/test` runs and exits 0. The matrix tests in § 2 all pass through both `cargo st` (interpreter) and the AOT path (LLVM). `timeout 150 ./test-all.sh` produces green output with no regressions. The let-polymorphism test `test_let_polymorphism_for_lambda` (verifying `let id = x -> x; id(1); id("hello")` still works) continues to pass, confirming the fix preserves polymorphism for function types.
