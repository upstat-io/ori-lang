---
section: "02"
title: "Type Checker"
status: in-progress
goal: "Track and resolve all known type checker bugs"
sections: []
third_party_review:
  status: findings
  updated: 2026-03-31
---

# Section 02: Type Checker

**Subsystem:** `compiler/ori_types/`

Bugs in type inference, unification, trait resolution, method dispatch, generics, bounds checking, and error reporting.

---

## Open Bugs

- [ ] `[BUG-02-001][high]` **infer_if() allows non-void then-branch without else** — found by verify-roadmap.
  Repro: `@main () -> void = { if true then 42 }` — compiles without error, but spec requires `if` without `else` to have type `void` or `Never` in the then-branch.
  Subsystem: `compiler/ori_types/src/infer/expr/control_flow.rs` (lines 53-63)
  Found: 2026-03-28 | Source: verify-roadmap
  Note: Active work in roadmap section 10 (control flow) touches this area.

- [x] `[BUG-02-002][high]` **Coalesce (`??`) same-type wrapper forms typed as `T` instead of wrapper** — found by tpr-review.
  Repro: `let a: Option<int> = Some(1); let b: Option<int> = Some(2); let c: Option<int> = a ?? b;` — fails with `expected int?, found int`. Same for `Result<T,E> ?? Result<T,E>`.
  Spec: `operator-rules.md` §Coalesce defines `Option<T> ?? Option<T> -> Option<T>` and `Result<T,E> ?? Result<T,E> -> Result<T,E>`.
  Resolved: Fixed on 2026-03-30 (typeck: 3a9bf319, codegen: 11a26626, tests: 51bcabd4). Type checker uses Idx identity (pool interning) to detect same-type wrapper chaining. ARC lowering uses `lhs_ty == result_ty` to distinguish chaining from nested unwrap (`Option<Option<T>> ?? Option<T>`). 11 positive spec tests + 2 negative compile_fail pins + 3 Rust unit tests + 14 AOT tests for LLVM dual-execution parity.

---

## 02.R Third Party Review Findings

- [ ] `[TPR-02-004][high]` `compiler/ori_types/src/infer/expr/operators.rs:318` — Coalesce still accepts invalid RHS wrapper types when the unwrap-path unification fails.
  Evidence: the fallback unwrap branch discards `engine.unify_types(inner, right_ty)` errors and returns `engine.resolve(inner)` anyway. Fresh repro on 2026-03-31: `let a: Option<Option<int>> = Some(Some(1)); let b: Result<int, bool> = Err(false); let c: Option<int> = a ?? b;` compiles successfully under `timeout 150 cargo run -q -p oric --bin ori -- check /tmp/coalesce_invalid_option_rhs.ori`, and typed IR reports `Binary(??) : int?`. `operator-rules.md §Coalesce` only permits `Option<T> ?? T` and `Option<T> ?? Option<T>` for `Option`, so `Option<Option<int>> ?? Result<int, bool>` should be rejected. The same silent acceptance occurs for `Result<Option<int>, str> ?? Result<int, bool>`, which the current tree infers as `int?`.
  Impact: the reviewed BUG-02-002 / TPR-02-003 work still leaves `??` unsound for nested-wrapper mismatch cases while the section marks the coalesce bug resolved, and the new negative coverage in `tests/spec/expressions/coalesce.ori` only pins the simplest `Option<int> ?? Result<int, str>` mismatch.
  Required plan update: make the unwrap path report a type error when unification with the RHS fails instead of discarding the error, then add negative pins for nested-wrapper mismatches on both `Option` and `Result` plus matching Rust and AOT regression coverage.

- [x] `[TPR-02-001][medium]` `plans/bug-tracker/section-02-typeck.md:25` — BUG-02-002 is marked resolved even though the new coalesce regression coverage was not verified on LLVM/AOT.
  Evidence: this range adds wrapper-chain and negative-pin cases in `tests/spec/expressions/coalesce.ori:434` and `tests/spec/expressions/coalesce.ori:568`, but `timeout 150 ./target/debug/ori test --backend=llvm tests/spec/expressions/coalesce.ori` still reports `42 llvm compile fail` and skips the file. The pre-change file also failed under LLVM (`31 llvm compile fail` on `HEAD~3`), so this is a carried-forward verification gap rather than a new backend regression.
  Impact: the type checker fix is covered by Rust unit tests and interpreter-side spec tests, but it still violates the repo's required dual-execution parity for compiler bug fixes; the new coalesce semantics are not pinned on the LLVM path.
  Resolved: Fixed on 2026-03-31. Added 14 AOT tests in `compiler/ori_llvm/tests/aot/coalesce.rs` covering both unwrap forms (Option/Result) and same-type wrapper chaining (BUG-02-002 regression pins) through the LLVM codegen path. All 14 pass. The spec test file's LLVM failure is a systemic `assert_eq` resolution issue (pre-existing, affects many spec files), not a coalesce-specific gap.

- [x] `[TPR-02-002][low]` `plans/bug-tracker/section-02-typeck.md:28` — The BUG-02-002 resolution note documents the superseded ARC heuristic and stale test totals rather than the final tree.
  Evidence: the note says ARC lowering passes through the LHS whenever the result type is `Option/Result` and claims `12 new spec tests + 3 Rust unit tests`, but the final code in `compiler/ori_arc/src/lower/expr/mod.rs:435` uses `lhs_ty == result_ty` to avoid misclassifying `Option<Option<T>> ?? Option<T>`, and the full reviewed range adds 14 spec tests.
  Impact: the authoritative bug-tracker entry now describes an intermediate implementation that was already corrected by `11a26626`, which obscures the real codegen condition and the cross-section nature of the fix.
  Resolved: Fixed on 2026-03-31. Updated BUG-02-002 resolution note with correct `lhs_ty == result_ty` logic, accurate commit references, and correct test inventory (11 positive + 2 negative + 3 unit + 14 AOT).

- [x] `[TPR-02-003][high]` `compiler/ori_types/src/infer/expr/operators.rs:292` — Coalesce wrapper-chain inference still rejects polymorphic RHS constructors like `None` and `Err(...)`, so BUG-02-002 is not actually fixed for the full `COALESCE-CHAIN` / `COALESCE-RESULT-CHAIN` surface.
  Evidence: `infer_binary()` decides chain vs unwrap by comparing `resolve(left_ty)` and `resolve(right_ty)` before it has unified the RHS wrapper with the LHS wrapper. That works for explicitly typed RHS operands, but it misclassifies constructor-polymorphic wrappers. Repros on the current tree: `let a = Some(1); let b: Option<int> = a ?? None;` fails with `expected int?, found int`, and `let a = Ok(1); let b: Result<int, str> = a ?? Err("x");` fails with `expected result<int, str>, found int`.
  Resolved: Fixed on 2026-03-31. Changed chain detection from Idx identity to tag-based detection with unification: when both sides have the same wrapper tag (Option/Result), try `unify_types(left, right)` — if it succeeds, it's CHAIN; if it fails (e.g. `Option<Option<int>> ?? Option<int>`), fall through to UNWRAP. Added 1 Rust unit test, 5 spec tests, 3 AOT tests. 14,794 tests pass.

---

## Resolved Bugs

- None.
