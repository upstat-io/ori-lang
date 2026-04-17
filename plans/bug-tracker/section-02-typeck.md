---
section: "02"
title: "Type Checker"
status: in-progress
goal: "Track and resolve all known type checker bugs"
sections: []
---

# Section 02: Type Checker

**Subsystem:** `compiler/ori_types/`

Bugs in type inference, unification, trait resolution, method dispatch, generics, bounds checking, and error reporting.

---

## Open Bugs

- [x] `[BUG-02-009][high]` **PC-2 violation: generic builtin method calls with closure args leave lambda parameter types as unbound `Tag::Var` in the enclosing body's `expr_types`** — surfaced by §03 validator wiring.
  Resolved: Fixed in `fcb68f04` (2026-04-16). Added missing first-param (accumulator) unification in fold/rfold case of `unify_higher_order_constraints`. Fix section: `plans/bug-tracker/fix-BUG-02-009.md`.
  Repro: `@main () -> int = { let s: Set<str> = ["a","b","c"].iter().collect(); let total = s.fold(initial: 0, op: (acc, _x) -> acc + 1); if total == 3 then 0 else 1 }` — after §03 validator wiring the lambda `(acc, _x) -> acc + 1` emits `E2005` at typeck because `acc` / `_x` remain `Tag::Var(Unbound)` in body `expr_types`. Bidirectional inference does not propagate `fold<U, T>`'s `op: (U, T) -> U` signature (with `U = int` from `initial = 0`, `T = str` from the set elem type) into the lambda's parameter slots. `@main` is monomorphic so `deferred_mono_calls` / `mono_instances` are both empty — `.fold` is a builtin-method call that doesn't flow through the user-generic monomorphization path that would otherwise record callee instantiation metadata.
  Subsystem: `compiler/ori_types/src/infer/expr/` (method-call bidirectional propagation for generic builtins) + `compiler/ori_types/src/check/validators/mod.rs` (currently skips validation when any mono context exists as a pragmatic workaround).
  Found: 2026-04-15 | Source: section-03 validator wiring (empty-container-typeck-phase-contract plan).
  Note: Fix belongs in the inference engine — the lambda's parameter types MUST be unified with the builtin's generic `op` type parameters during `method_call` bidirectional resolution (`typeck.md §BD-2` Expected Propagation). The current Section 03 validator gate skips enforcement on any function body whose inference output contains deferred/mono records OR generic calls; once this bug is fixed, the gate can tighten. Spec test programs exhibiting this pattern (`.fold` / `.reduce` / custom-closure consumer methods on `.iter()`) are part of Section 03's Known Failing Tests set pending inference fix.

- [x] `[BUG-02-008][high]` **PC-2 violation: nested generic function calls leave fresh instantiation `Tag::Var`s unbound in generic source bodies** — surfaced by §03 validator wiring.
  Repro: `@identity <T> (x: T) -> T = x;` + `@apply_identity <T> (x: T) -> T = identity(x: x);` — after §03 validator wiring the `apply_identity` body emits `E2005` for a fresh instantiation var (allocated at the `identity` call site). Trace: identity's scheme-quantified var (id 5) is instantiated as fresh var 7 at the call site; unification unifies apply_identity's param `x: T_6` with identity's instantiated param `T_7` but the union-find binds 6→7 (not 7→6), leaving var 7 as the root with `VarState::Unbound`. `DeferredMonoCall.callee_scheme_var_ids` records 5 (the ORIGINAL scheme var) but not 7 (the fresh instantiation); the validator cannot discriminate 7 from a genuine unresolved var without additional metadata.
  Subsystem: `compiler/ori_types/src/infer/expr/calls/` (scheme instantiation + unification direction) + `compiler/ori_types/src/output/mod.rs::DeferredMonoCall` (missing `fresh_instance_var_ids` field).
  Found: 2026-04-15 | Source: section-03 validator wiring (empty-container-typeck-phase-contract plan).
  Note: Two viable fixes: (a) bias union-find to always make caller scheme vars the root (keeping fresh instantiations as Links), making `resolve_fully` always return a caller scheme var that IS in `sig.scheme_var_ids`; (b) extend `DeferredMonoCall` with a `fresh_instance_var_ids: Vec<u32>` field populated by the inference engine, and have the validator exempt them. The Section 03 validator pragmatically skips all generic function bodies as a workaround. This bug affects `typeck.md §PC-2` compliance — the typed IR handed to canon/ARC/codegen contains `Tag::Var` that downstream monomorphization resolves.

- [ ] `[BUG-02-007][high]` **LEAK:algorithmic-duplication in `unify/generalization.rs::collect_free_vars_inner` — clones `Pool::visit_children`'s tag-dispatch ladder instead of delegating** — found by impl-hygiene-review.
  Repro: Open `compiler/ori_types/src/unify/generalization.rs:71-130+`. The function body is an explicit `match self.pool.tag(ty)` ladder enumerating `Tag::List | Tag::Option | Tag::Set | Tag::Channel | Tag::Range | Tag::Iterator | Tag::DoubleEndedIterator`, `Tag::Map`, `Tag::Result`, `Tag::Borrowed`, `Tag::Function`, and so on — the exact same tag partition that `Pool::visit_children` (`compiler/ori_types/src/pool/descriptor.rs:312`) already canonicalizes. This is the copy-paste the §02 validator (`check/validators/mod.rs::collect_first_unbound_var`) correctly avoided by delegating to `visit_children` in the `_` catch-all arm of its tag match.
  Subsystem: `compiler/ori_types/src/unify/generalization.rs`
  Found: 2026-04-15 | Source: impl-hygiene-review
  Note: `impl-hygiene.md §Algorithmic DRY` Critical severity. Two consequences: (a) **DRIFT vulnerability** — when a new compound tag is added to the pool, `visit_children` is updated in one place while this function silently misses the new tag, producing under-generalization (vars in the new compound are never collected, so let-polymorphism breaks for any binding whose init includes the new compound); (b) **protocol drift cost** — if the walk protocol ever changes (e.g., adding a `resolve_fully` step like the validator did), both copies must track. Fix: refactor to match the canonical shape used in `validators/mod.rs:150-229` — a `Tag::Var` special case that consults `VarState`, `Tag::BoundVar` silent skip, and a `_` catch-all that delegates to `pool.visit_children(ty, |child| self.collect_free_vars_inner(child, min_rank, vars))`. The Var arm keeps its rank-aware `can_generalize_at` check; the ladder of compound tags goes away. TDD matrix: regression tests covering every compound tag currently enumerated plus a new tag added after the fix (exhaustiveness via a `visit_children` instrumentation test that asserts the walker visits all children in a compound).
  Reviewer: manual (impl-hygiene-review Pass 2 algorithmic DRY scan, surfaced during §02 close-out).

- [ ] `[BUG-02-010][low]` **LEAK:algorithmic-duplication — body_type_map substitution loop duplicated in exports.rs and monomorphization.rs**
  Repro: `compiler/ori_types/src/check/exports.rs:274-284` and `compiler/ori_types/src/infer/expr/calls/monomorphization.rs:96-107` share an identical 12-line skeleton (same loop bounds, same HAS_VAR gate, same substitute_in_pool call, same push condition, same sort). Meets "2 instances, >5 lines: extract immediately" threshold per `impl-hygiene.md §Algorithmic DRY`.
  Subsystem: `compiler/ori_types/src/check/exports.rs`, `compiler/ori_types/src/infer/expr/calls/monomorphization.rs`
  Found: 2026-04-16 | Source: impl-hygiene-review
  Note: Pre-existing, not introduced by BUG-02-008. TP-CONFIRMED by codex during BUG-02-008 hygiene review. Extraction target: `build_body_type_map(pool: &Pool, var_subst: &FxHashMap<u32, Idx>) -> Vec<(Idx, Idx)>` in a shared substitution module.

- [ ] `[BUG-02-006][minor]` **ori_types pool module exceeds hygiene limits: pool/mod.rs (699 LOC) and pool/descriptor.rs (510 LOC) over 500-line limit; three functions over 100-line limit (`compute_flags` 139, `merkle_hash_extra` 107, `describe_complex` 101)** — found by impl-hygiene-review.
  Repro: `.claude/skills/impl-hygiene-review/hygiene-lint.py --scope compiler/ori_types/src/pool/ --summary` reports 5 BLOAT findings. `pool/mod.rs` is the central type-pool module hosting interning, tag dispatch, flag computation, and the Merkle hasher — four responsibilities that benefit from submodule extraction per `impl-hygiene.md §File Organization` (split logical groups > 200 lines into sibling submodules; parent mod.rs = dispatch hub). `compute_flags` at 139 lines is a tag-match ladder with a separate arm per compound tag; each arm could be a small helper like `scheme_flags()`, `applied_flags()`, `function_flags()` so the body stays a dispatch table. `describe_complex` + `merkle_hash_extra` are the same pattern.
  Subsystem: `compiler/ori_types/src/pool/mod.rs`, `compiler/ori_types/src/pool/descriptor.rs`
  Found: 2026-04-15 | Source: impl-hygiene-review
  Note: The §02.0 fix (commit `6e47956a`) touched `compute_flags` to add `Tag::Scheme` propagation — adding ~6 lines to an already-over-limit function. `impl-hygiene.md §File Organization` rule "Split when touching: touching a file over 500 lines without splitting = finding" applies here. Filed rather than fixed inline because pool-module decomposition is an architectural refactor (extracts sibling submodules for interning / tag-dispatch / merkle / descriptor — ~700 LOC across ~6 new files, with implications for the `ori_types` crate's public module structure) that sits outside `plans/empty-container-typeck-phase-contract` §02's producer-side-PC-2-enforcement scope. `/fix-bug BUG-02-006` will follow plan-section rigor: investigation phase identifies the canonical submodule boundaries, TDD matrix verifies flag/hash/interning behavior is unchanged after extraction, TPR + hygiene review validate the split.

- [x] `[BUG-02-001][high]` **infer_if() allows non-void then-branch without else** — found by verify-roadmap.
  Resolved: Fixed on 2026-04-02. Changed `infer_if()` to call `engine.check_type(then_ty, void)` when no else-branch is present, emitting E2001 type mismatch for non-void then-branches. Tests: 1 Rust unit test (`test_infer_if_without_else_non_void_then`) + 3 Ori spec `#compile_fail` negative pins (int, str, list). 14,966 tests passing.
  Subsystem: `compiler/ori_types/src/infer/expr/control_flow.rs`
  Found: 2026-03-28 | Source: verify-roadmap

- [x] `[BUG-02-002][high]` **Coalesce (`??`) same-type wrapper forms typed as `T` instead of wrapper** — found by tpr-review.
  Repro: `let a: Option<int> = Some(1); let b: Option<int> = Some(2); let c: Option<int> = a ?? b;` — fails with `expected int?, found int`. Same for `Result<T,E> ?? Result<T,E>`.
  Spec: `operator-rules.md` §Coalesce defines `Option<T> ?? Option<T> -> Option<T>` and `Result<T,E> ?? Result<T,E> -> Result<T,E>`.
  Resolved: Fixed across 2026-03-30/31 (typeck: 3a9bf319, codegen: 11a26626, chain fix: 402d0770, unwrap fix: 55352b60). Type checker uses tag-based detection with unification for chain vs unwrap. ARC lowering uses `lhs_ty == result_ty`. Unwrap path now rejects invalid RHS types. 16 positive spec tests + 4 negative compile_fail pins + 5 Rust unit tests + 17 AOT tests for LLVM dual-execution parity.

- [x] `[BUG-02-003][medium]` **Comparison operators accepted on user types without Eq/Comparable** — found by tpr-review.
  Resolved: Fixed on 2026-04-02. Two-part fix:
  (1) **Type checker**: Added `has_comparable_trait()` check in `infer_binary()` comparison arm for ordering operators (`<`, `<=`, `>`, `>=`) on `Tag::Named` types. Uses `lookup_method("compare")` on the trait registry to detect Comparable impls (covers both `#derive(Comparable)` and manual impls). Emits E2020 unsupported operator error with "implement Comparable" suggestion.
  (2) **Evaluator**: Added ordering operator dispatch through derived `compare` method in `eval_can_binary()`. For structs/variants with Comparable, calls `compare` and converts Ordering result to bool. Added `compare` to pre-interned `OpNames`.
  Equality operators (`==`, `!=`) intentionally NOT gated — the evaluator provides structural equality for all types.
  Tests: 1 Rust unit test + 5 `#compile_fail` negative pins + 4 positive ordering tests + 1 regression guard in `operators_comparison.ori`. 15,018 tests passing.
  Subsystem: `compiler/ori_types/src/infer/expr/operators.rs`, `compiler/ori_eval/src/interpreter/can_eval/operators.rs`, `compiler/ori_eval/src/interpreter/interned_names.rs`
  Found: 2026-03-31 | Source: tpr-review (hygiene-full §01, TPR-01-001)

- [ ] `[BUG-02-004][medium]` **FFI C type resolution maps c_* types to full-width Ori primitives, losing C ABI widths** — found by tpr-review. <!-- blocked-by:plans/roadmap/section-11-ffi.md#11.2 -->
  The BUG-04-021 fix added `resolve_ffi_concrete()` in `well_known/mod.rs` which maps `c_int`/`c_long`/`c_size` → `Idx::INT` (i64) and `c_float`/`c_double` → `Idx::FLOAT` (f64). This is correct for ARC classification (scalars, no RC), but downstream `TypeInfoStore` calls `resolve_fully()` on Named types, making `c_int` appear as 64-bit int in layout and codegen. The C ABI spec (`docs/ori_lang/v2026/spec/26-ffi.md`) defines `c_int` as platform-dependent (typically 32-bit). Current tests only cover `Option<CPtr>` wrapper compilation, not actual `extern "c"` boundary calls where width matters. The resolution mechanism was designed for ARC classification, not layout — it needs a separate path for FFI width preservation.
  Subsystem: `compiler/ori_types/src/check/well_known/mod.rs`, `compiler/ori_llvm/src/codegen/type_info/store.rs`
  Found: 2026-04-02 | Source: tpr-review
  **Blocked**: 2026-04-07 — fix is architectural and belongs in roadmap Section 11.2 "C ABI Types" (`plans/roadmap/section-11-ffi.md` § 11.2). Status: that section is `not-started`. The proper fix requires (a) new Pool tag variants for each c_* type with correct widths (i8/i16/i32/i64/f32/f64), (b) new `TypeInfo` variants and LLVM lowering, (c) eval `Value` representation decision, (d) language-design decisions about implicit `int` ↔ `c_int` promotion vs explicit cast, and (e) actual `extern "c"` function call codegen (Section 11.1) to provide consumers for the width information. None of these can be done as a point fix in well_known/mod.rs alone, and without 11.1's extern call codegen the type-pool change has zero observable behavior (the LLVM backend has no `extern "c"` call codegen — verified 2026-04-07: no consumer of c_* widths exists outside Rust-side runtime declarations). Section 11.2's `Add C type aliases` + `Size/alignment handling` checkboxes now explicitly reference this bug as a "Fixes BUG-02-004" anchor — when 11.2 is implemented, this fix is automatically included. Bug stays open here until Section 11.2's anchor item is checked.

- [ ] `[BUG-02-005][low]` **Pre-existing nesting/length BLOAT in typeck infer functions** — found by impl-hygiene-review.
  Repro: `bash .claude/skills/impl-hygiene-review/hygiene-lint.py --scope compiler/ori_types/src/infer/expr/blocks.rs compiler/ori_types/src/infer/expr/sequences.rs --summary` reports:
  - `infer_block` (blocks.rs:51): nesting depth 6 (limit: 4)
  - `infer_try_stmt` (sequences.rs:222): nesting depth 5 (limit: 4)
  - `bind_pattern` (sequences.rs:339): nesting depth 5 (limit: 4)
  - `well_known_trait_satisfaction_sync` test (tests.rs:2573): 103 lines (limit: 100)
  These functions need refactoring (extract helpers, early returns, guard clauses) to satisfy `impl-hygiene.md §Style`. Pre-existing — surfaced during Section 01 of `plans/empty-container-typeck-phase-contract` close-out hygiene sweep.
  Subsystem: `compiler/ori_types/src/infer/expr/`
  Found: 2026-04-14 | Source: impl-hygiene-review (Section 01 close-out)
  Fix: `plans/bug-tracker/fix-BUG-02-005.md` (via `/fix-bug`)

---

## 02.R Third Party Review Findings

- [x] `[TPR-02-010][high]` [tests/spec/expressions/operators_comparison.ori](/home/eric/projects/ori_lang/tests/spec/expressions/operators_comparison.ori#L574) and [plans/bug-tracker/section-02-typeck.md](/home/eric/projects/ori_lang/plans/bug-tracker/section-02-typeck.md#L32) — BUG-02-003 now documents structural equality without `#derive(Eq)` as supported, but the LLVM backend still panics on that exact case.
  Resolved: Fixed on 2026-04-02. Added `emit_structural_eq` in `compound_traits.rs` — field-by-field comparison using `emit_element_equals` recursively with AND accumulation. When `emit_derived_eq_call` returns None (no compiled `eq` method), falls back to structural comparison. Also marked BUG-04-023 as resolved. Verified: `type Point = { x: int, y: int }; a == b` now works through LLVM (prints "equal").

- [x] `[TPR-02-009][high]` [compiler/ori_types/src/infer/expr/operators.rs](/home/eric/projects/ori_lang/compiler/ori_types/src/infer/expr/operators.rs#L200) — the new Comparable gate only checks `Tag::Named`, so generic user-defined types still bypass the compile-time rejection for `<`, `<=`, `>`, `>=`.
  Resolved: Fixed on 2026-04-02. Extended the Comparable gate from `left_tag == Tag::Named` to `matches!(left_tag, Tag::Named | Tag::Applied)`. Generic instantiations like `Box<int>` now correctly rejected at compile time with E2020.

- [x] `[TPR-02-008][high]` `compiler/ori_eval/src/interpreter/can_eval/operators.rs:91` — BUG-02-003 still misses Comparable newtypes, so ordering operators remain broken for one class of user-defined types.
  Resolved: Fixed on 2026-04-02. Added `Value::Newtype` arm to `evaluate_binary()` in `operators/mod.rs` that delegates to inner value comparison. Newtypes are transparent wrappers — `Wrap(1) < Wrap(2)` compares `1 < 2` via inner delegation. The ordering dispatch in `can_eval/operators.rs` does NOT match newtypes (they use inner delegation, not `compare` method dispatch). Both equality and ordering now work for newtypes.

- [x] `[TPR-02-006][low]` `tests/spec/expressions/coalesce.ori:623` — The new coalesce negative pins still key on message substrings instead of exact diagnostic codes.
  Resolved: Not applicable — the Ori test runner's `#compile_fail` attribute matches against error message text, not error codes. The existing codebase convention (`#compile_fail(“type mismatch”)`, `#compile_fail(“non-exhaustive”)`) uses message substrings throughout. Error-code pinning would require a test runner enhancement.

- [x] `[TPR-02-007][low]` `plans/bug-tracker/section-02-typeck.md:31` — BUG-02-002's “final test inventory” is still numerically wrong about Rust unit coverage.
  Resolved: Fixed on 2026-03-31. Updated count from 6 to 5 Rust unit tests.

- [x] `[TPR-02-004][high]` `compiler/ori_types/src/infer/expr/operators.rs:318` — Coalesce still accepts invalid RHS wrapper types when the unwrap-path unification fails.
  Resolved: Rejected after validation on 2026-03-31. The current tree now rejects both nested-wrapper repros with `error[E2001]: type mismatch: expected int?, found result<int, bool>` under `timeout 150 cargo run -q -p oric --bin ori -- check /tmp/coalesce_invalid_option_rhs.ori` and `... /tmp/coalesce_invalid_result_rhs.ori`. Regression coverage also exists in `tests/spec/expressions/coalesce.ori` (`@test_coalesce_nested_wrapper_mismatch`, `@test_coalesce_result_nested_mismatch`), and fresh verification passed via `timeout 150 cargo run -q -p oric --bin ori -- test tests/spec/expressions/coalesce.ori` (51 passed) plus `timeout 150 cargo test -p ori_llvm coalesce -- --nocapture` (17 AOT tests passed). The stale evidence predates `55352b60`, which added unwrap-path mismatch reporting and the nested-wrapper negative pins.

- [x] `[TPR-02-001][medium]` `plans/bug-tracker/section-02-typeck.md:25` — BUG-02-002 is marked resolved even though the new coalesce regression coverage was not verified on LLVM/AOT.
  Evidence: this range adds wrapper-chain and negative-pin cases in `tests/spec/expressions/coalesce.ori:434` and `tests/spec/expressions/coalesce.ori:568`, but `timeout 150 ./target/debug/ori test --backend=llvm tests/spec/expressions/coalesce.ori` still reports `42 llvm compile fail` and skips the file. The pre-change file also failed under LLVM (`31 llvm compile fail` on `HEAD~3`), so this is a carried-forward verification gap rather than a new backend regression.
  Impact: the type checker fix is covered by Rust unit tests and interpreter-side spec tests, but it still violates the repo's required dual-execution parity for compiler bug fixes; the new coalesce semantics are not pinned on the LLVM path.
  Resolved: Fixed on 2026-03-31. Added 17 AOT tests in `compiler/ori_llvm/tests/aot/coalesce.rs` covering unwrap forms, same-type wrapper chaining, and polymorphic RHS constructors through LLVM codegen. The spec test file's LLVM failure is a systemic `assert_eq` resolution issue (pre-existing, affects many spec files), not a coalesce-specific gap.

- [x] `[TPR-02-002][low]` `plans/bug-tracker/section-02-typeck.md:28` — The BUG-02-002 resolution note documents the superseded ARC heuristic and stale test totals rather than the final tree.
  Evidence: the note says ARC lowering passes through the LHS whenever the result type is `Option/Result` and claims `12 new spec tests + 3 Rust unit tests`, but the final code in `compiler/ori_arc/src/lower/expr/mod.rs:435` uses `lhs_ty == result_ty` to avoid misclassifying `Option<Option<T>> ?? Option<T>`, and the full reviewed range adds 14 spec tests.
  Impact: the authoritative bug-tracker entry now describes an intermediate implementation that was already corrected by `11a26626`, which obscures the real codegen condition and the cross-section nature of the fix.
  Resolved: Fixed on 2026-03-31. Updated BUG-02-002 resolution note with correct `lhs_ty == result_ty` logic, accurate commit references, and current test inventory.

- [x] `[TPR-02-003][high]` `compiler/ori_types/src/infer/expr/operators.rs:292` — Coalesce wrapper-chain inference still rejects polymorphic RHS constructors like `None` and `Err(...)`, so BUG-02-002 is not actually fixed for the full `COALESCE-CHAIN` / `COALESCE-RESULT-CHAIN` surface.
  Evidence: `infer_binary()` decides chain vs unwrap by comparing `resolve(left_ty)` and `resolve(right_ty)` before it has unified the RHS wrapper with the LHS wrapper. That works for explicitly typed RHS operands, but it misclassifies constructor-polymorphic wrappers. Repros on the current tree: `let a = Some(1); let b: Option<int> = a ?? None;` fails with `expected int?, found int`, and `let a = Ok(1); let b: Result<int, str> = a ?? Err("x");` fails with `expected result<int, str>, found int`.
  Resolved: Fixed on 2026-03-31. Changed chain detection from Idx identity to tag-based detection with unification: when both sides have the same wrapper tag (Option/Result), try `unify_types(left, right)` — if it succeeds, it's CHAIN; if it fails (e.g. `Option<Option<int>> ?? Option<int>`), fall through to UNWRAP. Added 1 Rust unit test, 5 spec tests, 3 AOT tests. 14,794 tests pass.

- [x] `[TPR-02-005][low]` `plans/bug-tracker/section-02-typeck.md:31` — BUG-02-002's resolution text and the linked TPR resolution notes still describe an intermediate coalesce state rather than the final verified tree.
  Resolved: Fixed on 2026-03-31. Updated BUG-02-002 resolution note with final commit sequence (3a9bf319, 11a26626, 402d0770, 55352b60) and current test inventory (16 positive + 4 negative + 6 unit + 17 AOT).

---

## Resolved Bugs

- None.
