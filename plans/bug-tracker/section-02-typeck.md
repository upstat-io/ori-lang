---
section: "02"
title: "Type Checker"
status: in-progress
goal: "Track and resolve all known type checker bugs"
sections: []
third_party_review:
  status: findings
  updated: 2026-04-02
---

# Section 02: Type Checker

**Subsystem:** `compiler/ori_types/`

Bugs in type inference, unification, trait resolution, method dispatch, generics, bounds checking, and error reporting.

---

## Open Bugs

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

- [ ] `[BUG-02-004][medium]` **FFI C type resolution maps c_* types to full-width Ori primitives, losing C ABI widths** — found by tpr-review.
  The BUG-04-021 fix added `resolve_ffi_concrete()` in `well_known/mod.rs` which maps `c_int`/`c_long`/`c_size` → `Idx::INT` (i64) and `c_float`/`c_double` → `Idx::FLOAT` (f64). This is correct for ARC classification (scalars, no RC), but downstream `TypeInfoStore` calls `resolve_fully()` on Named types, making `c_int` appear as 64-bit int in layout and codegen. The C ABI spec (`docs/ori_lang/v2026/spec/26-ffi.md`) defines `c_int` as platform-dependent (typically 32-bit). Current tests only cover `Option<CPtr>` wrapper compilation, not actual `extern "c"` boundary calls where width matters. The resolution mechanism was designed for ARC classification, not layout — it needs a separate path for FFI width preservation.
  Subsystem: `compiler/ori_types/src/check/well_known/mod.rs`, `compiler/ori_llvm/src/codegen/type_info/store.rs`
  Found: 2026-04-02 | Source: tpr-review
  Note: Investigated 2026-04-06 — bug is **latent**. The LLVM backend has no `extern "c"` function call codegen. `resolve_ffi_concrete()` is only consumed for ARC classification (correct: all C types are scalars). No code path currently emits C function calls through LLVM, so the width loss causes no observable incorrect behavior. Fix becomes meaningful when FFI codegen is added to the LLVM backend (requires `TypeInfoStore` or a new FFI type layer to map Named c_* types to correct LLVM types: i8/i16/i32/i64/float/double).

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
