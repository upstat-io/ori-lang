---
bug: "BUG-04-074"
title: "AOT codegen: empty list literal `[]` with `push()` leaves unresolved type variables — LLVM verification failure"
severity: "high"
status: in-progress
resume_point: "Phase 2.5 Plan TPR round 3 — the round-2-revised plan needs to be re-reviewed after substantial architectural changes. Round 2 run: /tmp/ori-tpr-G6VzNAHW (codex 424s, gemini 223s, 12 actionable findings — all resolved in §R Phase 2.5 Round 2). Critical round-2 architectural changes: (1) validator moved from per-let-site to per-body-exit sweep — §3.3 entirely rewritten; (2) has_unbound_var now calls pool.resolve_fully() first to walk VarState::Link chains; (3) pseudocode rewritten against real Pool accessors (function_params, applied_args, struct_field_types, map_key/map_value, scheme_vars/scheme_body) replacing non-existent pool.children / pool.extra; (4) ambiguous_type constructor signature corrected to (span, var_id, context_desc: String); (5) engine.has_errors() cascade gate dropped — only TypeFlags::HAS_ERROR local gate remains; (6) scope broadened honestly — validator is type-agnostic, [] is spec-sanctioned, {}/Set ride free; (7) invalid id([]) positive pin replaced with id(1) + id('hello') (concrete instantiations) and id([]) + annotation; (8) generic-function interaction test added; (9) §3.6 debug_assert at codegen entry added. Re-run command: invoke /tpr-review with objective 'Re-review the BUG-04-074 fix plan at plans/bug-tracker/fix-BUG-04-074.md after round 2 revisions. Focus on §3.3 (architectural rewrite: per-body-exit timing, resolve_fully-first, real Pool accessors, correct constructor signature, tightened cascade gate); §1.5 (broadened-scope rationale); §2 (new generic-function interaction test + fixed lambda pins); §3.6 (new debug_assert); §R Phase 2.5 Round 2 resolutions. Strict mode — probe: do the real Pool accessors enumerated in §3.3 all exist on Pool (function_params, function_return, tuple_elems, applied_args, struct_field_types, enum_variant_payloads, map_key, map_value, result_ok, result_err, scheme_vars, scheme_body)? Is per-body-exit the right timing, or do some let bindings (e.g., lambda param bindings, match arm bindings, for-yield accumulators) need per-scope-exit granularity? Does resolve_fully's 16-hop limit cause issues with deeply-nested Link chains? Is the Cascade-suppression-only-via-HAS_ERROR behavior diagnostic-correct (i.e., when an earlier unrelated error poisons the binding transitively, is HAS_ERROR actually set, or does the transitive taint not propagate)?' Continue /fix-bug workflow from Phase 3 (TDD) once Plan TPR is clean."
goal: "Empty list literals (`[]`) with element types inferred solely from downstream usage compile cleanly through AOT, with `resolve_fully()` producing concrete element types for codegen. Ambiguous empty-list bindings emit spec-mandated E2005 (14-expressions.md:1224-1228) at type check time, not codegen."
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

### Final agreed approach (REVISED after Plan TPR rounds 1 & 2 — 2026-04-14)

**Multi-part point fix** (still scoped to `ori_types` crate, no plan escalation needed). Revised after TWO rounds of Plan TPR findings — round 1 reshaped consensus (§R Phase 2.5 round 1), round 2 corrected architectural flaws in the validation approach (§R Phase 2.5 round 2).

1. **AST-based Value Restriction at all 3 generalization sites**. Extract a shared helper `should_generalize(arena: &ExprArena, init: ExprId) -> bool` that returns true ONLY if the init is a non-capturing `ExprKind::Lambda`. Apply at:
   - `compiler/ori_types/src/infer/expr/blocks.rs:79-89` — block-statement let in `infer_block` (primary repro path per TPR-04-004-codex)
   - `compiler/ori_types/src/infer/expr/blocks.rs:167` — `infer_let` (standalone let expression — rare surface, needs dedicated test)
   - `compiler/ori_types/src/infer/expr/sequences.rs:247` — try-block let

   This SOLVES the primary repro: `ages.push(value: 10)` directly unifies the fresh Var(X) with `int` via `VarState::Link` (no generalization interposed), so `pool.resolve_fully()` resolves `List(Var(X))` → `List(int)` downstream.

2. **Per-body-exit validation pass emitting E2005** (REVISED round 2 per TPR-04-001-codex-r2 and cascading changes):

   **Do NOT validate at `bind_pattern` time.** At that point, later statements in the same block have not yet run, so downstream constraints (`push(10)` linking the element var) have not fired. Validating then emits E2005 for the success-case repro.

   **Correct timing**: validation runs as a sweep at the END of each Bodies-group pass (passes 2–5 per `typeck.md CK-1`) — after the full function body / method body / test body has been inferred. By body exit, all in-scope unification has fired; a `Tag::Var` still unresolved at that point is genuinely ambiguous.

   **Implementation shape**:
   - The checker records every let-binding's `(span, final_ty)` pair into a per-body side table during body inference (one vector per body).
   - At body-exit, walk the side table and invoke `validate_binding_type_resolved(engine, span, final_ty)` for each entry.
   - `validate_binding_type_resolved`:
     a. Call `pool.resolve_fully(final_ty)` first to follow `VarState::Link` chains (per `pool/accessors.rs:412-491`). Without this, resolved-but-unsubstituted vars false-positive as ambiguous.
     b. Cascade-suppress: skip if `pool.flags(resolved).contains(TypeFlags::HAS_ERROR)` — the binding is already poisoned and emitting a second diagnostic violates `typeck.md UN-4` monotonicity. Do NOT use `engine.has_errors()` — that is module-wide and silently swallows E2005 when an unrelated prior error exists.
     c. Invoke `has_unbound_var(pool, resolved, bound_vars: &FxHashSet<u32>)` — returns `Option<OffendingVar { var_id, context_desc }>`.
     d. On `Some(offender)`: `engine.push_error(TypeCheckError::ambiguous_type(span, offender.var_id, offender.context_desc))` — matches the real constructor signature at `check_error/mod.rs:236`.

   The `has_unbound_var` recursion walks the resolved type using the REAL tag-specific Pool accessors:
   - Simple containers (Tag 16..32 — List, Option, Set, Range, Iterator, DoubleEndedIterator): child via `Idx::from_raw(pool.data(idx))` per `types.md TK-1`.
   - Two-child (Map, Result): `pool.map_key(idx)` + `pool.map_value(idx)` (or `pool.result_ok(idx)` + `pool.result_err(idx)`).
   - Complex (Function, Tuple, Struct, Enum): `pool.function_params(idx)` + `pool.function_return(idx)`; `pool.tuple_elems(idx)`; `pool.struct_fields(idx)`; `pool.enum_variants(idx)`.
   - Named / Applied: `pool.applied_args(idx)` for generic args; Named unwraps to its underlying via `pool.resolve(idx)` before recursion.
   - Scheme: `pool.scheme_vars(idx)` are pushed into `bound_vars`, then recurse into `pool.scheme_body(idx)`, then vars are popped. A `Tag::Var(v)` encountered during the scheme body walk is RESOLVED iff `bound_vars.contains(&v)`; unresolved iff NOT.

3. **Scope broadened honestly** (REVISED round 2 per TPR-04-005-codex-r2):

   The validator is intentionally type-agnostic — it walks any `Tag::Var` in the resolved binding type and flags it when not scheme-bound. The spec at `14-expressions.md:1224-1228` mandates `[]` ambiguity rejection; `{}` and `Set<T>` ambiguity being rejected by the SAME mechanism is incidental (the mechanism does not special-case lists). This is architecturally correct per `impl-hygiene.md §SSOT` — special-casing by constructor tag would be a LEAK:scattered-knowledge hack.

   The TDD matrix (§2) focuses on `[]` as the spec-sanctioned case with ONE documentation-regression test for `{}` showing that the validator catches map ambiguity too (with a note that any future spec expansion is a SEPARATE proposal, not scope creep here). Sets are not tested because `Set<T>()` is not valid Ori surface syntax (verified: `infer_empty_set()` does not exist).

4. **Recursive negative pin** (per TPR-04-002-codex round 1): use `contains_var` (modeled after `ori_llvm/src/codegen/function_compiler/lambda_mono/type_predicates.rs:10-25`) to walk the repro's typed IR recursively and assert no unresolved Vars at any depth.

5. **Test updates broadened** (per TPR-04-005-codex round 1): audit includes `[].iter()`, `[].iter().rev().collect()`, `[].iter().last()`, `[].iter().rfind(...)`, `[].iter().rfold(...)` in `double_ended_methods.ori` lines 35, 84, 133, 174 — not just `let name = []` bindings.

6. **Algorithmic DRY follow-up**: file separately as BUG-04-{next} per TPR-04-003-gemini round 1. Consolidation of the three let-binding sites into `bind_local_let` is out of scope for this fix.

7. **Positive tests** (REVISED round 2 per TPR-04-006-codex-r2):
   - Repro: `let ages = []; ages = ages.push(value: 10); if ages.len() == 1 then 0 else 1` compiles via AOT.
   - Lambda let-polymorphism (TYPE-SAFE pin per round 2): `let id = x -> x; let a = id(1); let b = id("hello")` — the scheme `∀α. (α) -> α` is instantiated twice with different concrete types; both bindings type-check and neither is ambiguous. (Round-2 rejected the prior pin `let b = id([])` because `[]` alone provides no element-type context, so E2005 correctly fires on `b` — that pin would FAIL post-fix.)
   - Lambda polymorphism with annotated empty list: `let id = x -> x; let b: [int] = id([])` — `b`'s annotation resolves the ambiguity; `id` stays polymorphic. Added per TPR-04-006-codex-r2 to specifically test the "can I still pass `[]` through a polymorphic lambda" case with the annotation resolving ambiguity.
   - Capturing lambda: unchanged existing behavior.

8. **Negative pins**:
   - `let xs = []; xs.len()` — E2005 at check time (not codegen).
   - `let x = []; unknown_fn(x)` — only `UnknownIdent` error fires, NOT spurious E2005 (cascade-suppression pin per TPR-04-003-codex round 1 — round-2 tightened to rely ONLY on `TypeFlags::HAS_ERROR`, not module-wide `engine.has_errors()`).
   - Regression pin: recursive `contains_var` walk of repro's typed IR shows no unresolved Vars.
   - Generic-function interaction (NEW round 2 per TPR-04-005-gemini-r2): `@id<T> (x: T) -> T = x; let xs = []; id(xs)` — E2005 on `xs` because no downstream constraint on the element type even after the generic call.

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

### Lambda polymorphism regression pins (REVISED round 2)
- [ ] `test_polymorphic_lambda_instantiated_at_multiple_concrete_types_passes` — corrected pin per TPR-04-006-codex-r2: `let id = x -> x; let a = id(1); let b = id("hello")` — both instantiations concrete; neither ambiguous. If generalization still fires for lambdas after the fix, this test passes; if the fix over-reaches and disables lambda generalization, this test fails.
- [ ] `test_polymorphic_lambda_with_annotated_empty_list_resolves_cleanly` — NEW round 2: `let id = x -> x; let b: [int] = id([])` — verifies polymorphism survives AND the annotation on `b` resolves the ambiguity. Without the annotation, E2005 correctly fires — that's the negative case tested below.
- [ ] `test_polymorphic_lambda_captures_outer_ambiguous_empty_rejected` — important edge case: `{ let empty = []; let f = () -> empty; f() }` — the outer `empty` is ambiguous (len never constrains). After fix, E2005 on `empty`, because `empty` is not a lambda binding. The closure `f` capturing `empty` doesn't change `empty`'s ambiguity.

### Generic function interaction (NEW round 2 — per TPR-04-005-gemini-r2)
- [ ] `test_empty_list_passed_to_generic_function_emits_E2005` — `@id<T> (x: T) -> T = x; let xs = []; id(xs)` — the generic call instantiates `id` with `List(β)` where `β` is fresh and remains unlinked (there is no downstream use that constrains the element type). At body exit, `xs : List(β)` with `β` still `Unbound` → E2005 fires with the `β` var_id in the diagnostic. Explicitly covers `tests.md §Interaction Testing`'s mandated `[] passed to generic function` cell.

### Incidental map ambiguity coverage (NEW round 2 — documentation regression test)
- [ ] `test_empty_map_ambiguous_no_constraint_emits_E2005_documentation` — `let m = {}; m.len()` — the validator is type-agnostic and catches this the same way it catches `[]`. This test DOCUMENTS the incidental coverage (spec currently neutral on `{}`); the test file's `/// doc comment` explains: "Spec at `14-expressions.md:1224-1228` mandates `[]` ambiguity rejection. `{}` ambiguity rejection rides for free because the validator walks any `Tag::Var`. Spec expansion to cover `{}` explicitly is a separate proposal."

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

### 3.3 Per-body-exit validation pass (E2005 emission — REVISED 2026-04-14 ROUND 2)

**CRITICAL round-2 revision**: do NOT validate at `bind_pattern` time. That is too early — in `infer_block` (`blocks.rs:22-97`), later statements in the same block have not yet been inferred at the moment `bind_pattern` runs for a let binding. For the success-case repro `let ages = []; ages = ages.push(value: 10); ages.len()`, the `push(10)` unification that links `ages`'s element var runs AFTER `bind_pattern` for the let has completed. Validating at `bind_pattern` time would emit E2005 for the success case, breaking the stated success criterion.

**Correct approach**: validate at the END of each Bodies-group pass (passes 2–5 per `typeck.md CK-1`) — after the entire body has been inferred. By body exit, all in-scope unification has fired.

**Implementation shape**:

```rust
// Per-body side table populated during body inference.
// Lives on InferEngine or a per-body context; one Vec per body.
pub(crate) struct LetBindingRecord {
    pub span: Span,
    pub final_ty: Idx,
    // Optional: pattern info for better diagnostics (field name, etc.)
    pub pattern_label: Option<Name>,
}

// At each let-binding site in infer_block, infer_let, sequences.rs try-block let:
//   AFTER bind_pattern has been called, push the record onto the body's list.
engine.record_let_binding(LetBindingRecord {
    span: stmt.span,
    final_ty,
    pattern_label: pattern_first_name(arena, pat),  // already exists in blocks.rs:182
});
```

At the end of each Bodies-group pass (in `check/bodies/mod.rs` functions `check_function_bodies`, `check_test_bodies`, `check_impl_bodies`, `check_def_impl_bodies`), after the body's `infer_*` call returns but BEFORE releasing the per-body context, invoke:

```rust
/// Sweep the per-body let-binding records and emit E2005 for any binding
/// whose final type still carries an unresolved Tag::Var after all in-body
/// unification has fired.
///
/// Timing: MUST run at body exit, not at per-let-site bind time. See
/// TPR-04-001-codex-r2 — the per-let-site version false-positives on
/// the success-case repro because later statements in the body have
/// not yet unified the element var at the moment bind_pattern runs.
///
/// Cascade suppression: skip when the binding's final_ty carries
/// TypeFlags::HAS_ERROR. Per typeck.md UN-4, Tag::Error unifies with
/// anything silently; emitting a second diagnostic violates recovery
/// monotonicity. Do NOT use engine.has_errors() — that is module-wide
/// and over-suppresses when an unrelated prior error exists. Scoped
/// per-binding is the correct granularity.
///
/// Fix: BUG-04-074
pub(crate) fn validate_body_let_bindings(engine: &mut InferEngine<'_>, records: &[LetBindingRecord]) {
    for record in records {
        let resolved = engine.pool().resolve_fully(record.final_ty);

        // Cascade suppression — if this binding's type is already poisoned,
        // the root-cause diagnostic has already been emitted elsewhere.
        if engine.pool().flags(resolved).contains(TypeFlags::HAS_ERROR) {
            continue;
        }

        // Fast path — no variables means nothing to check.
        if !engine.pool().flags(resolved).contains(TypeFlags::HAS_VAR) {
            continue;
        }

        let mut bound = FxHashSet::default();
        if let Some(offender) = has_unbound_var(engine.pool(), resolved, &mut bound) {
            let context_desc = record
                .pattern_label
                .map(|n| format!("binding `{}`", engine.resolve_name(n)))
                .unwrap_or_else(|| "let binding".to_string());
            engine.push_error(TypeCheckError::ambiguous_type(
                record.span,
                offender.var_id,
                context_desc,
            ));
        }
    }
}

pub(crate) struct OffendingVar {
    pub var_id: u32,
}

/// Recursively find the first unresolved Tag::Var in `ty` whose var_id is NOT
/// bound by an enclosing Tag::Scheme.
///
/// CRITICAL: caller MUST pass a pool.resolve_fully(ty) result, not a raw Idx.
/// resolve_fully walks VarState::Link chains (pool/accessors.rs:412-491);
/// without this prefix step, resolved-but-unsubstituted vars false-positive
/// as ambiguous.
///
/// When a Tag::Scheme is encountered, its scheme_vars are added to `bound`
/// for the recursion into its body, then removed on exit (stack discipline).
/// Nested schemes push/pop correctly: inner scheme's vars shadow outer's
/// only if var_ids collide (which they never do — fresh_var allocates globally
/// unique var_ids per EN-3).
fn has_unbound_var(pool: &Pool, ty: Idx, bound: &mut FxHashSet<u32>) -> Option<OffendingVar> {
    // Fast path: no vars at any depth.
    if !pool.flags(ty).contains(TypeFlags::HAS_VAR) {
        return None;
    }

    match pool.tag(ty) {
        Tag::Var => {
            let var_id = pool.data(ty);
            if bound.contains(&var_id) {
                None
            } else {
                Some(OffendingVar { var_id })
            }
        }
        // BoundVar (inside a scheme body) is always "bound" — it refers to the
        // scheme's binder, not an unresolved inference var.
        Tag::BoundVar => None,
        // RigidVar is a user-annotated generic parameter; parametric per TK-6
        // and never ambiguous at this use site.
        Tag::RigidVar => None,
        Tag::Scheme => {
            let scheme_vars: Vec<u32> = pool.scheme_vars(ty).to_vec();
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
        // Simple containers — child is in `data`, not `extra`. Per types.md TK-1
        // simple-container row: `data = child_idx.raw()`.
        Tag::List | Tag::Option | Tag::Set | Tag::Range
        | Tag::Iterator | Tag::DoubleEndedIterator | Tag::Channel => {
            has_unbound_var(pool, Idx::from_raw(pool.data(ty)), bound)
        }
        // Two-child containers — use the dedicated accessors.
        Tag::Map => {
            has_unbound_var(pool, pool.map_key(ty), bound)
                .or_else(|| has_unbound_var(pool, pool.map_value(ty), bound))
        }
        Tag::Result => {
            has_unbound_var(pool, pool.result_ok(ty), bound)
                .or_else(|| has_unbound_var(pool, pool.result_err(ty), bound))
        }
        // Complex types — use the dedicated accessors.
        Tag::Function => {
            for param in pool.function_params(ty) {
                if let Some(off) = has_unbound_var(pool, *param, bound) {
                    return Some(off);
                }
            }
            has_unbound_var(pool, pool.function_return(ty), bound)
        }
        Tag::Tuple => {
            for elem in pool.tuple_elems(ty) {
                if let Some(off) = has_unbound_var(pool, *elem, bound) {
                    return Some(off);
                }
            }
            None
        }
        Tag::Struct => {
            for field_ty in pool.struct_field_types(ty) {
                if let Some(off) = has_unbound_var(pool, *field_ty, bound) {
                    return Some(off);
                }
            }
            None
        }
        Tag::Enum => {
            for variant_payload in pool.enum_variant_payloads(ty) {
                if let Some(off) = has_unbound_var(pool, *variant_payload, bound) {
                    return Some(off);
                }
            }
            None
        }
        Tag::Applied => {
            for arg in pool.applied_args(ty) {
                if let Some(off) = has_unbound_var(pool, *arg, bound) {
                    return Some(off);
                }
            }
            None
        }
        // Named resolves via registry; if unresolved at this point, it is already
        // an earlier diagnostic's problem. Skip silently (cascade).
        Tag::Named | Tag::Alias => None,
        // Projection should have been normalized by unify.rs UN-8 or flagged
        // by the checker before this sweep runs. Skip silently.
        Tag::Projection => None,
        // Primitives, Error, Unit, Never, SelfType (already substituted), Infer
        // (replaced by Var at CK-3) — none contain unresolved vars at the
        // relevant layer.
        _ => None,
    }
}
```

**Key differences from round-1 pseudocode**:

1. **Timing moved from bind_pattern time to body exit** — the single most important fix per TPR-04-001-codex-r2.
2. **`resolve_fully` is mandatory at entry** — `validate_body_let_bindings` calls `pool.resolve_fully(record.final_ty)` BEFORE inspecting the type. This walks `VarState::Link` chains (pool/accessors.rs:412-491) so resolved inference vars aren't false-positive'd. Addresses TPR-04-002-codex-r2 + TPR-04-001-gemini-r2.
3. **Real Pool accessors** — replaced non-existent `pool.children(ty)` and `pool.extra(off)` (both missing or private) with the concrete per-tag accessors enumerated in `types.md TL-2` and `TK-1`. Addresses TPR-04-004-codex-r2 + TPR-04-003-gemini-r2.
4. **Correct `ambiguous_type` constructor signature** — `(span, var_id, context_desc: String)` per `check_error/mod.rs:236`, not `(span)`. `engine.push_error` not `engine.record_error`. Return type of `has_unbound_var` is `Option<OffendingVar>` not `bool` so the caller can construct the diagnostic. Addresses TPR-04-002-gemini-r2.
5. **Cascade gate tightened** — dropped the module-wide `engine.has_errors()` branch; kept ONLY the `TypeFlags::HAS_ERROR` local check on the specific binding's type. Addresses TPR-04-003-codex-r2 + TPR-04-004-gemini-r2.
6. **Scheme handling preserved** — scheme_vars push/pop stack discipline works for nested schemes (fresh_var allocates globally unique var_ids per EN-3, so no shadowing collision risk). `Tag::BoundVar` is always treated as resolved (it's a scheme binder reference, not an inference var).
7. **Explicit handling for every Tag range** — primitives, BoundVar, RigidVar, Named, Alias, Projection all return `None` explicitly; Scheme handles bound-var tracking; simple containers use `data`-backed child; two-child and complex types use dedicated accessors.

**Helper module location** (preserved from round 1):
- Place `should_generalize` (§3.1) + `validate_body_let_bindings` + `has_unbound_var` + `OffendingVar` + `LetBindingRecord` in a new module `compiler/ori_types/src/infer/expr/generalization_policy.rs`.
- Use `pub(crate)` visibility so `infer_block`, `infer_let`, `sequences.rs`, and `check/bodies/mod.rs` can all call across sibling module boundaries.

**Pool accessor verification** — the round-2 revision depends on these accessors existing on `Pool`. BEFORE implementation, verify each via `scripts/intel-query.sh symbols "<name>" --repo ori`:
- `resolve_fully`, `map_key`, `map_value`, `result_ok`, `result_err` — verified round 2 via `pool/accessors.rs` inspection (lines 412-491 for `resolve_fully`).
- `function_params`, `function_return`, `tuple_elems`, `applied_args`, `scheme_vars`, `scheme_body` — verify during Phase 3 TDD; add them to Pool if missing (they are the natural shape of `types.md TL-2` complex-type extra layouts).
- `struct_field_types`, `enum_variant_payloads` — similarly verify; these return the type `Idx` slices from the registered struct/enum definitions.

If any accessor is missing, add it as a thin wrapper over `pool.extra` reads in the same commit as this fix — adding a missing typed accessor is NOT scope creep; it's the natural completion of the `types.md TL-2` contract. Any such accessor addition MUST have a corresponding `#[cfg(test)]` unit test in `ori_types/src/pool/tests.rs`.

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

### 3.6 Consumer-side invariant check at LLVM codegen entry (NEW round 2 — per TPR-04-007-codex-r2)

Add a `debug_assert!` at the entry to `ori_llvm::codegen::type_info::store::get_or_compute_type_info` (around `compiler/ori_llvm/src/codegen/type_info/store.rs:341`) that panics if a `Tag::Var` survives to codegen. Per `impl-hygiene.md §Cross-Phase Invariant Contracts`, every typed-IR-to-codegen contract (`typeck.md PC-2` + `types.md PC-2`: "No `Tag::Var` in any type position") SHOULD be `debug_assert!`'d at the consumer's entry. This makes the invariant explicit and prevents silent miscompilation in the rare case where a future bug lets a `Var` leak past the validator.

```rust
// In get_or_compute_type_info (or its innermost resolution point):
let resolved = pool.resolve_fully(idx);
debug_assert!(
    pool.tag(resolved) != Tag::Var,
    "unresolved Tag::Var reached LLVM codegen — typeck.md PC-2 violation. \
     idx={:?}, resolved={:?}, var_id={}",
    idx, resolved, pool.data(resolved),
);
```

The existing release-mode `TypeInfo::Error` return path is RETAINED as a defense-in-depth safety net — production builds still fail gracefully with an error rather than generating wrong LLVM IR, even if the `debug_assert!` is stripped. The `debug_assert!` is additive: it turns a silent-miscompile-in-release into a loud-crash-in-debug during development, which is the correct tradeoff per `impl-hygiene.md §Invariant Explicitness`.

### 3.7 Implementation order

1. Write all tests from §2 (including negative pins that REQUIRE E2005 emission)
2. Verify tests fail appropriately (some fail on codegen error, some pass trivially — document which)
3. Verify the Pool accessors enumerated in §3.3 exist (grep / intel-query.sh); add any missing ones in the same commit with dedicated unit tests
4. Implement §3.1 helper + §3.2 three-site replacement → verifies the main repro compiles end-to-end (typeck passes, codegen succeeds)
5. Implement §3.3 body-exit validation sweep → verifies ambiguous cases produce E2005 at check
6. Implement §3.6 consumer-side debug_assert → catches any future leak of Tag::Var to codegen
7. Apply §3.4 test updates as needed
8. Run `timeout 150 ./test-all.sh` — full suite green
9. Run `timeout 150 ./clippy-all.sh` — no warnings
10. Run `/commit-push`, then Phase 5 (TPR + hygiene)

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

### Phase 2.5 — Plan TPR Round 2 (2026-04-14)

Plan TPR round 2 run `/tmp/ori-tpr-G6VzNAHW`. Codex walltime 424s (131 events, 14096-byte envelope). Gemini walltime 223s (97 events, 5721-byte envelope). ASYMMETRY MODERATE (bytes 11.7x RED; walltime 1.9x and events 1.4x LOW). 12 actionable findings, 0 agreements (all complementary) — round-1 revisions introduced new architectural flaws in §3.3 that round 2 correctly surfaced. Every finding independently verified against the code; the critical claims (validator timing, Link-chain following, Pool API surface, `ambiguous_type` signature) all hold.

- [x] `[TPR-04-001-codex-r2][high]` `plans/bug-tracker/fix-BUG-04-074.md:320` — Validator runs too early; would emit E2005 on the success-case repro.
  Resolved: Revised §3.3 on 2026-04-14 (round-2). Validation moved from "immediately after `bind_pattern` at each let site" to a **per-body-exit sweep** invoked by the Bodies-group pass (CK-1 passes 2–5) after the full body has been inferred. By that point, downstream constraints (e.g., `push(10)` linking the element var) have had a chance to fire.
  Evidence: `compiler/ori_types/src/infer/expr/blocks.rs:22-97` processes block statements in order. For the repro `let ages = []; ages = ages.push(value: 10); ages.len()`, at the moment `bind_pattern` is called for `let ages = []`, the later `ages.push(value: 10)` has NOT yet unified the element var with `int`. Emitting E2005 at that point would fail the stated success criterion "the exact repro compiles via `ori build` and runs successfully."
  Required plan update: Change §3.3 to run validation at body exit (once per function / method / test body in Bodies-group passes 2–5), not at `bind_pattern` time.
  Basis: fresh_verification. Confidence: high. (Verified: `blocks.rs:22-97` block statement ordering confirmed.)

- [x] `[TPR-04-002-codex-r2][high]` `plans/bug-tracker/fix-BUG-04-074.md:384` — `has_unbound_var` must follow `VarState::Link` chains before classifying ambiguity.
  Resolved: Revised §3.3 on 2026-04-14. `has_unbound_var` now calls `pool.resolve_fully(ty)` as its first step (which walks `VarState::Link` chains up to 16 hops per `pool/accessors.rs:412-491`), then inspects the resolved type. A `Tag::Var` still present after `resolve_fully` is genuinely `Unbound` or `Generalized`-not-in-scope.
  Evidence: `compiler/ori_types/src/unify/substitute.rs:73-76` confirms: `if let VarState::Link { target } = self.pool.var_state(var_id) { return self.substitute(*target, subst); }` — a `Tag::Var` node with `VarState::Link` targets is a *resolved* inference variable. `pool/accessors.rs:412-491` `resolve_fully` walks Link chains explicitly. A walker that only checks `pool.tag(ty) == Tag::Var` would flag every resolved-but-unsubstituted var as ambiguous, producing false-positive E2005 for fully-inferred programs.
  Required plan update: Specify that `has_unbound_var` must invoke `pool.resolve_fully(ty)` before any structural dispatch.
  Basis: fresh_verification. Confidence: high.

- [x] `[TPR-04-001-gemini-r2][high]` `plans/bug-tracker/fix-BUG-04-074.md:386` — Resolve types before checking for variables to prevent false positives.
  Resolved: Same fix as [TPR-04-002-codex-r2] — `resolve_fully()` invoked first in `has_unbound_var`. Gemini's finding and codex's round-2-002 surfaced the same root cause via independent paths.
  Evidence: Same as [TPR-04-002-codex-r2].
  Required plan update: Same as [TPR-04-002-codex-r2].
  Basis: fresh_verification. Confidence: high. (Cross-reference: [TPR-04-002-codex-r2] — agreement-by-root-cause, different specific wording.)

- [x] `[TPR-04-002-gemini-r2][high]` `plans/bug-tracker/fix-BUG-04-074.md:387` — `ambiguous_type` constructor signature mismatch; code will fail to compile.
  Resolved: Revised §3.3 on 2026-04-14. Changed `has_unbound_var` return type from `bool` to `Option<OffendingVar>` where `OffendingVar { var_id: u32, context_desc: String }`. At the emission site: `engine.push_error(TypeCheckError::ambiguous_type(span, offender.var_id, offender.context_desc))`.
  Evidence: `compiler/ori_types/src/type_error/check_error/mod.rs:236`: `pub fn ambiguous_type(span: Span, var_id: u32, context_desc: String) -> Self`. The plan's `TypeCheckError::ambiguous_type(span)` does not match this signature. Additionally, `compiler/ori_types/src/infer/context.rs:68` defines `pub fn push_error(&mut self, error: TypeCheckError)` — the plan's `engine.record_error(...)` is not a real method.
  Required plan update: Fix the call-site to use the real 3-argument constructor signature and `push_error` method name; have `has_unbound_var` return the offending var_id so the caller can construct the diagnostic.
  Basis: fresh_verification. Confidence: high.

- [x] `[TPR-04-003-codex-r2][medium]` `plans/bug-tracker/fix-BUG-04-074.md:125` — Drop module-wide `engine.has_errors()` suppression.
  Resolved: Revised §3.3 on 2026-04-14. Removed the global `engine.has_errors()` branch. Cascade suppression now relies SOLELY on the type-local `pool.flags(final_ty).contains(TypeFlags::HAS_ERROR)` check per `TypeFlags::HAS_ERROR` propagation (`types.md TF-3`).
  Evidence: `compiler/ori_types/src/infer/context.rs:51-55`: `pub fn has_errors(&self) -> bool { !self.errors.is_empty() }`. This is module-wide — returns true if ANY prior error exists anywhere in the current module check. Under the proposed global gate, a pre-existing unrelated error ANYWHERE in the module silently swallows E2005 emissions for all subsequent ambiguous empty-list bindings, violating the success criterion "error surfaces at typeck, not codegen." The correct cascade-suppression gate is `TypeFlags::HAS_ERROR` on the binding's own `final_ty` — it propagates from children upward per `types.md TF-3`.
  Required plan update: Delete the `if engine.has_errors() { return; }` branch from `validate_binding_type_resolved`. Keep ONLY the `pool.flags(final_ty).contains(TypeFlags::HAS_ERROR)` check.
  Basis: fresh_verification. Confidence: high. (Cross-reference: [TPR-04-004-gemini-r2] — agreement on the over-suppression semantics.)

- [x] `[TPR-04-004-gemini-r2][medium]` `plans/bug-tracker/fix-BUG-04-074.md:374` — Remove global error suppression to avoid masking independent errors.
  Resolved: Same fix as [TPR-04-003-codex-r2] — drop `engine.has_errors()` global gate; retain only `TypeFlags::HAS_ERROR` local check.
  Evidence: Same as [TPR-04-003-codex-r2].
  Required plan update: Same as [TPR-04-003-codex-r2].
  Basis: fresh_verification. Confidence: high.

- [x] `[TPR-04-004-codex-r2][medium]` `plans/bug-tracker/fix-BUG-04-074.md:415` — Pseudocode references APIs that do not exist; must rewrite against real Pool accessors.
  Resolved: Revised §3.3 on 2026-04-14. Replaced `pool.children(ty)` + `pool.extra(extra_off)` with the real tag-specific accessors:
  - `Tag::Function` → `pool.function_params(idx)` + `pool.function_return(idx)`
  - `Tag::Tuple` → `pool.tuple_elems(idx)`
  - `Tag::Applied` → `pool.applied_args(idx)` (generic args for `Option<[]>`, `Result<[], E>`, etc.)
  - `Tag::Struct` → `pool.struct_fields(idx)` (field tys)
  - `Tag::Enum` → `pool.enum_variants(idx)` (variant payloads)
  - `Tag::Map` / `Tag::Result` → `pool.map_key(idx)` + `pool.map_value(idx)` (or `pool.result_ok(idx)` + `pool.result_err(idx)`)
  - `Tag::List` / `Tag::Option` / `Tag::Set` / `Tag::Range` / `Tag::Iterator` / `Tag::DoubleEndedIterator` → `Idx::from_raw(pool.data(idx))` (child in DATA per `types.md TK-1` simple-container row)
  - `Tag::Scheme` → `pool.scheme_vars(idx)` + `pool.scheme_body(idx)`
  Evidence: `pool.children()` does NOT exist as a public method on `Pool`; `compiler/ori_types/src/pool/descriptor.rs:299-360` has only private `visit_children()`. `pool.extra()` is private (the `extra` field is private to `Pool`). `types.md TK-1` confirms the simple-container vs extra-backed split; `types.md TL-2` enumerates the complex-type layouts. The plan's pseudocode as written won't compile.
  Required plan update: Rewrite §3.3 recursion using the concrete tag-specific accessors enumerated above.
  Basis: fresh_verification. Confidence: high. (Cross-reference: [TPR-04-003-gemini-r2] — gemini independently flagged the same missing APIs.)

- [x] `[TPR-04-003-gemini-r2][high]` `plans/bug-tracker/fix-BUG-04-074.md:417` — Use correct accessors for complex type children instead of non-existent `children` method.
  Resolved: Same fix as [TPR-04-004-codex-r2] — replaced placeholder API references with real tag-specific accessors.
  Evidence: Same as [TPR-04-004-codex-r2].
  Required plan update: Same as [TPR-04-004-codex-r2].
  Basis: fresh_verification. Confidence: high.

- [x] `[TPR-04-005-codex-r2][medium]` `plans/bug-tracker/fix-BUG-04-074.md:129` — Align implementation with the claimed list-only scope (or broaden scope honestly).
  Resolved: Revised §1.5 on 2026-04-14. Chose the BROADEN-HONESTLY path: the fix's mechanics are naturally type-agnostic (the generalization guard disables for any non-lambda init; the validator walks any unresolved `Tag::Var`). Rather than special-casing by constructor (which would be a LEAK:scattered-knowledge hack per `impl-hygiene.md §SSOT`), §1.5 now documents that the validator catches ALL ambiguous let-binding types as a uniform safety net. The spec currently (`14-expressions.md:1224-1228`) only mandates `[]` ambiguity rejection, but `{}` and `Set<T>` ambiguity being rejected by the same mechanism is a happy coincidence — not a special case. If the spec should be expanded to explicitly cover `{}` and `Set<T>`, that's a spec proposal tracked separately. The TDD matrix tests focus on `[]` (spec-sanctioned) plus one `{}` regression test to document the incidental coverage.
  Evidence: `plans/bug-tracker/fix-BUG-04-074.md:129-133` claimed "scope NARROWED to empty lists only" — but §3.1/§3.2 disable generalization for EVERY non-lambda init, and §3.3's recursive walker checks ANY unresolved `Tag::Var`. The stated narrowing conflicts with the implemented mechanics.
  Required plan update: Either (a) honestly broaden the stated scope to match the mechanics, or (b) actually special-case by tag. Option (a) is more correct architecturally because (b) would introduce ad-hoc per-tag logic that violates SSOT.
  Basis: direct_file_inspection. Confidence: high.

- [x] `[TPR-04-006-codex-r2][medium]` `plans/bug-tracker/fix-BUG-04-074.md:200` — Replace the invalid `id([])` lambda pin.
  Resolved: Revised §2 on 2026-04-14. The previous positive pin `let id = x -> x; let a = id([1, 2]); let b = id([])` had a logical flaw: `let b = id([])` provides NO element-type context for `[]`, so under the new semantics E2005 correctly fires for `b` — making the "pin" a false positive generator. Replaced with `let id = x -> x; let a = id([1, 2]); let b: [int] = id([])` where the annotation on `b` resolves the ambiguity, so the lambda remains polymorphic AND both applications type-check cleanly.
  Evidence: `compiler/ori_types/src/infer/expr/collections.rs:18-20` shows `[]` infers as `List(fresh_var)`. `let b = id([])` unifies `id`'s instantiation `(α) -> α` with `(List(β)) -> List(β)` — β is still unconstrained after the call, so `b : List(β)` with β unlinked. Under the fix, E2005 fires at body-exit on `b`. The plan's pin would FAIL post-fix, defeating its purpose as a polymorphism-preservation test.
  Required plan update: Change the pin to annotate `b` (`let b: [int] = id([])`) or otherwise constrain the element type downstream (e.g., `let b = id([]).push(value: 1)`).
  Basis: direct_file_inspection. Confidence: high.

- [x] `[TPR-04-005-gemini-r2][medium]` `plans/bug-tracker/fix-BUG-04-074.md:195` — Include generic function parameter interaction test in the matrix.
  Resolved: Added new test to §2 on 2026-04-14: `test_empty_list_passed_to_generic_function_emits_E2005`. Test case: `@id<T> (x: T) -> T = x; let xs = []; id(xs)` — no downstream constraint on `xs`'s element type, so E2005 must fire at body-exit. This verifies the `Tag::Var` survives generic unification (the generic `T` unifies with `List<β>` but β remains free) and is correctly flagged.
  Evidence: `tests.md §Interaction Testing` mandates 3 cross-feature tests, with "[] passed to generic function" explicitly listed. The round-1 matrix omitted this interaction.
  Required plan update: Added matrix row in §2 under "Cross-generalization-site coverage" + "Lambda let-polymorphism preservation" combined coverage.
  Basis: direct_file_inspection. Confidence: high.

- [x] `[TPR-04-007-codex-r2][low]` `compiler/ori_llvm/src/codegen/type_info/store.rs:351` — Add an explicit consumer-side invariant check.
  Resolved: Added §3.6 on 2026-04-14 (new subsection). The fix adds a `debug_assert!` at the LLVM codegen entry for `get_or_compute_type_info` that panics if a `Tag::Var` survives to codegen. Per `impl-hygiene.md §Cross-Phase Invariant Contracts`, every typed-IR-to-codegen contract should be `debug_assert!`'d at the consumer's entry. Production behavior unchanged (the existing `TypeInfo::Error` path is retained as a release-build safety net).
  Evidence: `compiler/ori_llvm/src/codegen/type_info/store.rs:341-363` currently logs `unresolved type variable at codegen — type inference bug` but does not `debug_assert!`. Per `CLAUDE.md §Stabilization Discipline`, implicit invariants become invisible regressions — every cross-phase invariant should be either a test or a `debug_assert!`. This fix makes the typeck→codegen "no Tag::Var" contract explicit per `types.md PC-2` and `typeck.md PC-2`.
  Required plan update: Added §3.6 specifying the `debug_assert!` at the codegen entry point.
  Basis: direct_file_inspection. Confidence: medium.

### Round 2 revisions applied

Key architectural changes beyond round 1:

1. **Validator placement moved from per-let-site to per-body-exit** (addresses [TPR-04-001-codex-r2]): the most critical revision. Validation is now a sweep at the end of each Bodies-group pass (passes 2–5 per `typeck.md CK-1`) over all let-binding types recorded during that body. This gives downstream unification constraints (e.g., `push(10)`) time to fire before validation.
2. **`resolve_fully` is mandatory** (addresses [TPR-04-002-codex-r2] + [TPR-04-001-gemini-r2]): `has_unbound_var` calls `pool.resolve_fully(ty)` as its first step, walking `VarState::Link` chains per `pool/accessors.rs:412-491`. Without this, resolved-but-unsubstituted vars are false-positive'd as ambiguous.
3. **Real Pool API surface** (addresses [TPR-04-004-codex-r2] + [TPR-04-003-gemini-r2]): pseudocode rewritten against concrete tag-specific accessors (`function_params`, `applied_args`, `struct_fields`, `map_key`/`map_value`, `scheme_vars`/`scheme_body`, data-backed `Idx::from_raw(pool.data(idx))` for simple containers).
4. **Constructor + method-name fixes** (addresses [TPR-04-002-gemini-r2]): `ambiguous_type(span, var_id, context_desc)` signature honored; `engine.push_error(...)` not `record_error`. `has_unbound_var` return type changed to `Option<OffendingVar>` so the caller can build the diagnostic.
5. **Cascade gate tightened** (addresses [TPR-04-003-codex-r2] + [TPR-04-004-gemini-r2]): dropped module-wide `engine.has_errors()`; kept ONLY `pool.flags(final_ty).contains(TypeFlags::HAS_ERROR)` local gate.
6. **Scope broadened honestly** (addresses [TPR-04-005-codex-r2]): §1.5 now documents that the validator catches ALL ambiguous let-binding types uniformly; `[]` is the spec-sanctioned case, `{}` and `Set<T>` ride for free without special-casing.
7. **Invalid lambda pin replaced** (addresses [TPR-04-006-codex-r2]): `let b: [int] = id([])` instead of the unconstrained `let b = id([])`.
8. **Generic-function interaction test added** (addresses [TPR-04-005-gemini-r2]): new `test_empty_list_passed_to_generic_function_emits_E2005`.
9. **Debug_assert at codegen entry** (addresses [TPR-04-007-codex-r2]): new §3.6 makes the typeck→codegen PC-2 contract explicit.

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
