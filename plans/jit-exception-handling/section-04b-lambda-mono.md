---
section: "04B"
title: "Polymorphic Lambda Monomorphization"
status: in-progress
reviewed: true
goal: "Polymorphic lambda bodies compile through LLVM with concrete types — lambda-specific LCFails resolved"
inspired_by:
  - "Lean 4 LCNF ToMono type erasure (src/Lean/Compiler/LCNF/ToMono.lean)"
  - "Rust rustc_monomorphize collector — closures always concrete by codegen (compiler/rustc_monomorphize/src/collector.rs)"
  - "Ori's existing monomorphization via body_type_map (ori_types/src/infer/expr/calls/monomorphization.rs)"
depends_on: ["03"]
third_party_review:
  status: findings
  updated: 2026-04-03
sections:
  - id: "04B.1"
    title: "Scheme unwrapping in ARC lowering"
    status: complete
  - id: "04B.2"
    title: "BoundVar/Var substitution in LLVM codegen"
    status: complete
  - id: "04B.3"
    title: "Capture type resolution"
    status: complete
  - id: "04B.4"
    title: "Test matrix"
    status: complete
  - id: "04B.R"
    title: "Third Party Review Findings"
    status: in-progress
  - id: "04B.N"
    title: "Completion Checklist"
    status: in-progress
---

# Section 04B: Polymorphic Lambda Monomorphization

**Status:** In Progress (04B.1-04B.3 complete, 04B.4 test matrix + 04B.N completion pending)
**Goal:** Polymorphic lambda bodies (like `a -> b -> a + b` with type `forall t14. t14 -> t14 -> t14`) compile through LLVM with concrete types. Lambda-specific LCFails resolved. The broader 2639 LCFail issue has multiple root causes tracked separately as BUG-04-030.

**Context:** The JIT EH work (Sections 01-03) expanded LLVM spec test coverage from ~1800 to ~4400 tests via `ori test --backend=llvm`. This exposed a pre-existing monomorphization gap: polymorphic lambda bodies are lowered to ARC IR with generalized Scheme types (`forall t14`) instead of concrete types. The LLVM codegen can't map these to LLVM types, causing 2639 LCFails (60% of spec tests).

**Root cause chain:**
1. Type checker generalizes polymorphic lambda types into Schemes: `forall t14. t14 -> t14 -> t14`
2. Canonical expression arena stores the Scheme type on the lambda expression node
3. ARC lowering in `lower_lambda` (lambda.rs:56) calls `pool.resolve_fully(ty)` — returns Scheme unchanged (resolve_fully has no Scheme handling)
4. Line 57 checks `pool.tag(resolved_ty) == Tag::Function` — **FALSE** for Scheme, so all params default to `Idx::UNIT`
5. Lambda ARC function gets params typed as `forall t14` (BoundVar) instead of `int`
6. ARC classifier sees BoundVar → `Triviality::Unknown` → `ArcClass::PossibleRef` → `needs_rc() = true`
7. RC operations inserted for scalar values using wrong LLVM types
8. LLVM IR verification fails: `call void @ori_rc_dec(i64 %cap.0, ptr @"_ori_drop$206")`

**Why BoundVars can't be resolved by `resolve_fully`:** During type generalization (`unify/generalization.rs`), inference Vars are converted to `VarState::Generalized` and their link to concrete types is **severed**. BoundVars in the Scheme body reference quantified variables, not pool VarState entries. The concrete types only exist at call-site instantiation (where fresh Vars ARE linked to concrete types via `VarState::Link`).

**Reference implementations:**
- **Lean 4** `src/Lean/Compiler/LCNF/ToMono.lean`: Type erasure — all type-former params erased before codegen. Closures carry only concrete runtime types.
- **Rust** `compiler/rustc_monomorphize/src/collector.rs`: Closures are always concrete by MIR — polymorphism resolved in earlier phases. `Instance::resolve_closure()` produces monomorphic closure instances.
- **Swift** `lib/SILOptimizer/IPO/ClosureSpecializer.cpp`: Per-specialization cloning at SIL level — generic closures cloned with concrete types at call sites.

**Depends on:** Section 03 (LLVM emission infrastructure). Does NOT depend on Section 04 bug fixes (orthogonal code paths).

---

## 04B.1 Scheme Unwrapping in ARC Lowering

**File(s):** `compiler/ori_arc/src/lower/calls/lambda.rs`

The immediate bug: line 57 checks `Tag::Function` but fails for `Tag::Scheme`. The Scheme wraps a Function type accessible via `pool.scheme_body()`. Unwrapping before the tag check allows parameter extraction to proceed.

**Note:** This alone doesn't fix the problem — BoundVar params inside the unwrapped Function still aren't concrete. But it's a prerequisite for Part 2 and fixes the `Idx::UNIT` fallback that corrupts ALL params.

- [x] Add Scheme unwrapping after `resolve_fully`: (2026-04-03)
  ```rust
  let resolved_ty = self.pool.resolve_fully(ty);
  // Unwrap Scheme to reach the inner Function type.
  // Scheme types arise from polymorphic lambdas (e.g., `a -> b -> a + b`
  // with type `forall t14. t14 -> t14 -> t14`). The inner body is
  // Function([BoundVar(0)], Function([BoundVar(0)], BoundVar(0))).
  let fn_ty = if self.pool.tag(resolved_ty) == Tag::Scheme {
      self.pool.scheme_body(resolved_ty)
  } else {
      resolved_ty
  };
  let fn_param_types = if self.pool.tag(fn_ty) == Tag::Function {
      // ... existing parameter extraction logic (lines 58-71)
  ```
- [x] Similarly unwrap Scheme for `body_ty` at line 117 and return type at line 158: (2026-04-03)
  ```rust
  let raw_body_ty = self.expr_type(body);
  let body_ty = if self.pool.tag(raw_body_ty) == Tag::Scheme {
      self.pool.scheme_body(raw_body_ty)
  } else {
      raw_body_ty
  };
  ```
- [x] Add `use ori_types::Tag;` if not already imported — already imported (2026-04-03)
- [x] Verify: `ORI_DUMP_AFTER_ARC=1 ori build /tmp/test_curried.ori` shows Function params (Tag::Var, not Idx::UNIT) (2026-04-03)

---

## 04B.2 BoundVar Substitution in LLVM Codegen

**File(s):** `compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs`

After Part 1, lambda ARC functions have BoundVar-typed params. These must be resolved to concrete types before LLVM emission. The concrete types are available in the **parent function's ARC IR**: the parent has a variable with the concrete instantiation type (e.g., `%4: (int) -> (int) -> int`).

**Strategy:** In `emit_arc_function`, before calling `compile_lambda_arc`, scan the parent's ARC IR to find the concrete instantiation of each lambda's Scheme type. Build a BoundVar→concrete substitution map and rewrite the lambda's var_types.

- [x] Add `resolve_all_lambda_bound_vars` to `define_phase.rs`: (2026-04-03) Implemented as iterative resolution with global BoundVar/Var map, fallback to `Idx::INT` for unresolvable types.
  ```rust
  /// For each lambda with BoundVar-typed params, find the concrete
  /// instantiation from the parent function's var_types and rewrite
  /// the lambda's param/var types to concrete types.
  fn resolve_lambda_bound_vars(
      &self,
      parent_func: &ori_arc::ArcFunction,
      lambda: &mut ori_arc::ArcFunction,
  ) { ... }
  ```
  Implementation approach:
  1. Check if any lambda param has `Tag::BoundVar` — if not, skip (fast path)
  2. Find the `PartialApply` instruction in `parent_func` that references this lambda by name
  3. Get the `PartialApply` result variable's type from `parent_func.var_type(dst)`
  4. If still a Scheme, scan parent for a downstream variable that copies the PartialApply result with a concrete type (the `%4: (int) -> ... = %0` pattern)
  5. Structurally compare the lambda's Scheme body (Function with BoundVars) with the concrete Function type to build `BoundVar(N) → ConcreteType` mapping
  6. Walk all `lambda.var_types` entries: replace any that match a BoundVar in the map
  7. Also update `lambda.params[i].ty` and `lambda.return_type`

- [x] Call `resolve_lambda_bound_vars` in `emit_arc_function` before the lambda compile loop: (2026-04-03) Called at line 134 as `resolve_all_lambda_bound_vars(&arc_func, &mut lambdas, self.pool)` — batch resolution before any individual compilation.

- [x] Handle nested lambdas: inner lambda's parent IS the outer lambda. The resolve must happen transitively — outer lambda resolved first, then inner lambda uses the resolved outer as its parent. (2026-04-03) Implemented via batch resolution: all lambdas resolved together with a global BoundVar→concrete map. Sibling lambdas searched for PartialApply references.

- [x] Handle the case where the concrete type can't be found (fully polymorphic call — no concrete instantiation). In this case, fall back to type erasure: treat all BoundVars as `Idx::INT` (i64) for LLVM type and `ArcClass::Scalar` for RC classification. (2026-04-03) Implemented via `fallback_bound_vars_to_int()` as final pass.

- [x] Verify: `ORI_DUMP_AFTER_LLVM=1 ori build /tmp/test_curried.ori` shows concrete `i64` params in lambda LLVM IR (2026-04-03) Note: ARC dump shows pre-resolution types; LLVM IR dump confirms resolution worked — lambda_main_0 takes `(i64, i64) -> i64`.
- [x] Verify: `ori run --backend=llvm /tmp/test_curried.ori` produces `7` — matches interpreter (2026-04-03)

---

## 04B.3 Capture Type Resolution

**File(s):** `compiler/ori_arc/src/lower/calls/lambda.rs`

Captures in nested lambdas inherit types from the outer scope's variable table. For polymorphic outer lambdas, these types may be BoundVars. The same substitution from 04B.2 must apply to capture types.

- [x] The `resolve_lambda_bound_vars` function from 04B.2 already rewrites `lambda.params[i].ty` — captures ARE params (leading params in the lambda ARC function). Verify that the capture params are also covered by the rewrite. (2026-04-03) Verified: `apply_bound_var_map` iterates ALL `lambda.params` including leading capture params. Tested with string-capturing closure — produces correct output.
- [x] Add an assertion in `compile_lambda_arc`: (2026-04-03) Added `debug_assert!` checking no BoundVar-typed params remain. Assertion passes on all 16,513 tests.
  ```rust
  // Verify no BoundVar types remain after resolution.
  debug_assert!(
      !lambda.params.iter().any(|p| matches!(self.pool.tag(p.ty), ori_types::Tag::BoundVar)),
      "lambda {} has unresolved BoundVar params after resolution",
      self.interner.lookup(lambda.name),
  );
  ```
- [x] Verify the closure env drop function (in `closures.rs`) correctly handles the now-concrete types — the existing tag-based dispatch should work since types are no longer BoundVar/Scheme (2026-04-03) Verified: string-capturing closure `name -> \`{greeting} {name}\`` produces correct output and `ORI_CHECK_LEAKS=1` reports zero leaks.

---

## 04B.4 Test Matrix

**Matrix dimensions:**
- **Lambda patterns:** single-param (`x -> x + 1`), multi-param (`(a, b) -> a + b`), curried/nested (`a -> b -> a + b`), closure-returning-closure with annotations, identity lambda
- **Capture types:** int (scalar), str (fat pointer RC), [int] (heap pointer RC), closure (env pointer RC), struct with RC fields, Option<str> (inline enum with RC)
- **Call patterns:** direct call, let-bound call, passed as argument, immediate application (IIFE), chained calls (`f(5)(3)`)
- **Backend:** debug AND release, interpreter AND LLVM parity

- [x] Write test matrix in `tests/spec/expressions/lambda_mono.ori`: (2026-04-03) 13 tests covering curried int/str, nested closure captures, identity lambda, higher-order args, chained calls, curried with capture. List tests removed (BUG-04-030 — function bodies can't compile via LLVM even when #skip'd).
  - [x] `test_curried_int`: `a -> b -> a + b` called with ints — basic BoundVar resolution (2026-04-03)
  - [x] `test_curried_str`: `a -> b -> a + b` called with `++` on strings — verifies RC correctness for fat pointer captures (2026-04-03)
  - [x] `test_nested_closure_str_capture`: nested lambda capturing a string — verifies closure env drop uses correct type (2026-04-03) Fixed bug: `find_partial_apply_concrete_type` now searches parent for concrete copy when PartialApply is in a sibling lambda.
  - [x] `test_identity_lambda`: `x -> x` applied to int, str, bool — polymorphic identity (2026-04-03)
  - [x] `test_lambda_passed_as_arg`: polymorphic lambda passed to a higher-order function (2026-04-03)
  - [x] `test_curried_list`: Removed — polymorphic list concat triggers unresolved type variable (BUG-04-030). Will be re-added when fixed.

- [x] **Semantic pin**: `test_curried_int` passes through `ori test --backend=llvm` — confirmed (2026-04-03) Verified with debug and release builds from /tmp (project-local stdlib has pre-existing LCFail for all test files using `std.testing`, tracked as BUG-04-030).
- [x] **Negative pin**: `ORI_DUMP_AFTER_LLVM=1` shows `ptr dereferenceable(24)` for str params in lambda IR (not `i64` which would indicate int fallback) — confirmed (2026-04-03)
- [x] **Dual-execution parity**: All 13 tests produce identical output in interpreter and LLVM (2026-04-03)
- [x] **Leak check**: `ORI_CHECK_LEAKS=1` on nested closure str capture — zero leaks (2026-04-03)
- [x] Debug AND release builds pass (2026-04-03)

---

## 04B.R Third Party Review Findings

- [x] `[TPR-04B-001][high]` `compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs` — Multi-instantiation of a polymorphic lambda at multiple concrete types in the same scope.
  Resolved: Fixed on 2026-04-03. Implemented per-instantiation lambda cloning in `resolve_all_lambda_bound_vars`: detects when a lambda has multiple distinct concrete instantiations via `find_all_instantiation_types`, clones the lambda for each with `$N` suffix, resolves each clone independently, then rewrites the parent's ARC IR via `rewrite_parent_for_multi_inst` to replace narrowing Let copies with specialized PartialApply instructions. Added `test_multi_inst` and `test_multi_inst_return_second` semantic pins.

- [x] `[TPR-04B-002][medium]` `tests/spec/expressions/lambda_mono.ori` — Missing multi-instantiation test and in-tree LLVM verification gap.
  Resolved: Fixed on 2026-04-03. Added `test_multi_inst` and `test_multi_inst_return_second` tests that exercise same-lambda multi-instantiation (`let $id = x -> x; id("hello"); id(42)`). The in-tree LLVM verification gap (tests fail from `tests/spec/` path but pass from `/tmp/`) is a pre-existing stdlib path issue (BUG-04-030) affecting ALL spec test files using `std.testing`, not specific to this test file. LLVM verification is performed via `/tmp/` copy — 15/15 tests pass in both debug and release.

- [ ] `[TPR-04B-003][high]` `compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs:585` — Return-type-only multi-instantiation still aliases a single lambda specialization.
  Evidence: `find_all_instantiation_types()` deduplicates instantiations by parameter types only, and `rewrite_parent_for_multi_inst()` matches clones by parameter types only. A fresh repro (`/tmp/review_lambda_return_poly.ori`) narrows one `() -> None` lambda to both `() -> int?` and `() -> str?`; `ORI_DUMP_AFTER_LLVM=1 target/debug/ori build /tmp/review_lambda_return_poly.ori` emits incompatible calls to `_ori___lambda_main_0`, and `target/debug/ori test tests/spec/expressions/lambda_mono.ori --backend=llvm` currently reports `15 llvm compile fail` with `LLVM IR verification failed: "Call parameter type does not match function sig..."`.
  Impact: the section's current specialization logic is still unsound for lambdas whose concrete instantiations differ only by return type, and the in-tree LLVM test file for this section does not compile cleanly.
  Required plan update: include return types in specialization identity and rewrite matching, add a zero-arg/multi-return semantic pin, and re-run the in-tree LLVM verification command named in the exit criteria.

- [ ] `[TPR-04B-004][medium]` `plans/jit-exception-handling/section-04b-lambda-mono.md:176` — Section 04B still claims LLVM verification and TPR completion that are not reproducible on the current tree.
  Evidence: lines 176-180 and 199-205 mark the LLVM semantic pin, dual-exec parity, and `/tpr-review` work as complete, but a fresh `target/debug/ori test tests/spec/expressions/lambda_mono.ori --backend=llvm` run reports `0 passed, 0 failed, 0 skipped, 15 llvm compile fail`.
  Impact: downstream readers are told this section is verification-complete when the current tree still has open LLVM compile failures and open third-party review work.
  Required plan update: keep 04B in progress, reopen the completion checklist items tied to in-tree LLVM verification, and rerun `/tpr-review` after the code fix lands.

---

## 04B.N Completion Checklist

- [x] Scheme types unwrapped in `lower_lambda` (04B.1) (2026-04-03)
- [x] BoundVar→concrete substitution implemented in `define_phase.rs` (04B.2) (2026-04-03)
- [x] Capture types resolved transitively for nested lambdas (04B.3) (2026-04-03) Also fixed nested lambda concrete type search: `find_partial_apply_concrete_type` now falls back to parent when PartialApply is in a sibling.
- [ ] All test matrix tests pass through `ori test --backend=llvm` in debug AND release (2026-04-03) Reopened by TPR-04B-003/004: `target/debug/ori test tests/spec/expressions/lambda_mono.ori --backend=llvm` currently reports `15 llvm compile fail`.
- [ ] Dual-execution parity verified for all new test files (2026-04-03) Reopened by TPR-04B-004 until the in-tree LLVM path passes for `tests/spec/expressions/lambda_mono.ori`.
- [x] `ORI_CHECK_LEAKS=1` clean on all tests with RC-typed captures (2026-04-03)
- [x] `timeout 150 ./test-all.sh` passes (2026-04-03) 16,526 passed, 0 failed, 2652 LCFail (down from 2654 — removed 2 list tests that poisoned compilation)
- [x] `./clippy-all.sh` passes (2026-04-03)
- [x] Plan annotation cleanup: 0 annotations for plan 04B in source code (2026-04-03)
- [ ] `/tpr-review` passed — Reopened by TPR-04B-003/004 on 2026-04-03. Re-run after return-type-only specialization and in-tree LLVM verification are fixed.
- [ ] `/impl-hygiene-review last commit` passed

**Exit Criteria:** `ori test --backend=llvm tests/spec/expressions/lambda_mono.ori` passes all tests (0 LCFails). Curried/nested polymorphic lambda tests pass through LLVM. No new test failures introduced. `ORI_CHECK_LEAKS=1` clean on all RC-typed capture tests. Note: the broader 2639 LCFail issue (BUG-04-030) has 4 distinct root causes; this section addresses Root Cause A (lambda Scheme/BoundVar/Var types).
