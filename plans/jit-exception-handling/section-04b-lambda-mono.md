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
  status: none
  updated: null
sections:
  - id: "04B.1"
    title: "Scheme unwrapping in ARC lowering"
    status: complete
  - id: "04B.2"
    title: "BoundVar/Var substitution in LLVM codegen"
    status: in-progress
  - id: "04B.3"
    title: "Capture type resolution"
    status: not-started
  - id: "04B.4"
    title: "Test matrix"
    status: not-started
  - id: "04B.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "04B.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 04B: Polymorphic Lambda Monomorphization

**Status:** In Progress (04B.1-04B.3 complete, 04B.4 test matrix pending)
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

- [ ] Call `resolve_lambda_bound_vars` in `emit_arc_function` before the lambda compile loop:
  ```rust
  for mut lambda in lambdas {
      self.resolve_lambda_bound_vars(&arc_func, &mut lambda);
      let original_name = lambda.name;
      self.compile_lambda_arc(&mut lambda);
      // ...
  }
  ```

- [ ] Handle nested lambdas: inner lambda's parent IS the outer lambda. The resolve must happen transitively — outer lambda resolved first, then inner lambda uses the resolved outer as its parent.

- [ ] Handle the case where the concrete type can't be found (fully polymorphic call — no concrete instantiation). In this case, fall back to type erasure: treat all BoundVars as `Idx::INT` (i64) for LLVM type and `ArcClass::Scalar` for RC classification. This is correct because:
  - Ori's runtime boxes all heap values; polymorphic code at runtime always works with i64-sized words
  - If the actual type were non-scalar (e.g., str), it would be boxed through a closure env, and the env's drop function (using concrete types from the call site) handles RC correctly

- [ ] Verify: `ORI_DUMP_AFTER_ARC=1 ori build /tmp/test_curried.ori` shows `int` params after resolution
- [ ] Verify: `ori test --backend=llvm /tmp/test_curried.ori` passes (no LCFail)

---

## 04B.3 Capture Type Resolution

**File(s):** `compiler/ori_arc/src/lower/calls/lambda.rs`

Captures in nested lambdas inherit types from the outer scope's variable table. For polymorphic outer lambdas, these types may be BoundVars. The same substitution from 04B.2 must apply to capture types.

- [ ] The `resolve_lambda_bound_vars` function from 04B.2 already rewrites `lambda.params[i].ty` — captures ARE params (leading params in the lambda ARC function). Verify that the capture params are also covered by the rewrite.
- [ ] Add an assertion in `compile_lambda_arc`:
  ```rust
  // Verify no BoundVar types remain after resolution.
  debug_assert!(
      !lambda.params.iter().any(|p| self.pool.tag(p.ty) == Tag::BoundVar),
      "lambda {} has unresolved BoundVar params after resolution",
      self.interner.lookup(lambda.name),
  );
  ```
- [ ] Verify the closure env drop function (in `closures.rs`) correctly handles the now-concrete types — the existing tag-based dispatch should work since types are no longer BoundVar/Scheme

---

## 04B.4 Test Matrix

**Matrix dimensions:**
- **Lambda patterns:** single-param (`x -> x + 1`), multi-param (`(a, b) -> a + b`), curried/nested (`a -> b -> a + b`), closure-returning-closure with annotations, identity lambda
- **Capture types:** int (scalar), str (fat pointer RC), [int] (heap pointer RC), closure (env pointer RC), struct with RC fields, Option<str> (inline enum with RC)
- **Call patterns:** direct call, let-bound call, passed as argument, immediate application (IIFE), chained calls (`f(5)(3)`)
- **Backend:** debug AND release, interpreter AND LLVM parity

- [ ] Write failing test matrix BEFORE implementation:
  ```ori
  // tests/spec/expressions/lambda_mono.ori
  // Tests that polymorphic lambdas compile correctly through LLVM
  ```
  - [ ] `test_curried_int`: `a -> b -> a + b` called with ints — basic BoundVar resolution
  - [ ] `test_curried_str`: `a -> b -> a + b` called with `++` on strings — verifies RC correctness for fat pointer captures
  - [ ] `test_nested_closure_str_capture`: nested lambda capturing a string — verifies closure env drop uses correct type
  - [ ] `test_identity_lambda`: `x -> x` applied to various types — polymorphic identity
  - [ ] `test_lambda_passed_as_arg`: polymorphic lambda passed to a higher-order function
  - [ ] `test_curried_list`: curried lambda operating on lists — heap pointer captures

- [ ] **Semantic pin**: `test_curried_int` MUST pass through `ori test --backend=llvm` — this test ONLY passes with the monomorphization fix (currently LCFails)
- [ ] **Negative pin**: Verify `ORI_DUMP_AFTER_ARC=1` shows concrete types (not `forall t14`) in lambda params after the fix
- [ ] **Dual-execution parity**: All tests produce identical output in interpreter and LLVM
- [ ] **Leak check**: `ORI_CHECK_LEAKS=1` on tests with str/list captures — zero leaks
- [ ] Debug AND release builds pass

---

## 04B.R Third Party Review Findings

- None.

---

## 04B.N Completion Checklist

- [ ] Scheme types unwrapped in `lower_lambda` (04B.1)
- [ ] BoundVar→concrete substitution implemented in `define_phase.rs` (04B.2)
- [ ] Capture types resolved transitively for nested lambdas (04B.3)
- [ ] All test matrix tests pass through `ori test --backend=llvm` in debug AND release
- [ ] Dual-execution parity verified for all new test files
- [ ] `ORI_CHECK_LEAKS=1` clean on all tests with RC-typed captures
- [ ] `timeout 150 ./test-all.sh` passes (LCFail count tracked separately as BUG-04-030 — this section fixes lambda-specific issues only)
- [ ] `./clippy-all.sh` passes
- [ ] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan 04B` returns 0 annotations
- [ ] `/tpr-review` passed — independent Codex review clean
- [ ] `/impl-hygiene-review last commit` passed

**Exit Criteria:** `ori test --backend=llvm tests/spec/expressions/lambda_mono.ori` passes all tests (0 LCFails). Curried/nested polymorphic lambda tests pass through LLVM. No new test failures introduced. `ORI_CHECK_LEAKS=1` clean on all RC-typed capture tests. Note: the broader 2639 LCFail issue (BUG-04-030) has 4 distinct root causes; this section addresses Root Cause A (lambda Scheme/BoundVar/Var types).
