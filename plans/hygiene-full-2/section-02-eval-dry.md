---
section: "02"
title: "Evaluator Algorithmic DRY"
status: not-started
reviewed: true
goal: "Extract shared control-flow skeletons from iterator consumers, Option/Result handlers, and method dispatch — eliminate 20+ algorithmic duplications"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "02.1"
    title: "Extract Iterator Consumer Drive Function"
    status: not-started
  - id: "02.2"
    title: "Extract Option/Result Method Handler"
    status: not-started
  - id: "02.3"
    title: "Unify Iterator Method Name Lists"
    status: not-started
  - id: "02.4"
    title: "Eliminate Redundant Name Interning"
    status: not-started
  - id: "02.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "02.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 02: Evaluator Algorithmic DRY

**Status:** Not Started
**Goal:** Extract shared control-flow skeletons into canonical helpers. Eliminate the 9-way iterator consumer duplication, 8-way Option/Result handler duplication, and 3-way iterator method name list duplication.
**Context:** The evaluator has the most algorithmic duplication of any crate. Nine iterator consumers share an identical `loop { eval_iter_next → match Some/None }` skeleton. Eight Option/Result method handlers share an identical `validate → extract → call closure → wrap` skeleton. Iterator method names are maintained in 3 independent lists that must be manually kept in sync.
---

## 02.1 Extract Iterator Consumer Drive Function

**File(s):** `compiler/ori_eval/src/interpreter/method_dispatch/iterator/consumers.rs`

Nine functions share identical loop harness: `eval_iter_fold`, `eval_iter_count`, `eval_iter_find`, `eval_iter_any`, `eval_iter_all`, `eval_iter_for_each`, `eval_iter_collect`, `eval_iter_collect_set`, `eval_iter_join`.
- [ ] Create `drive_iterator()` method on Interpreter:
  ```rust
  fn drive_iterator<A, F>(
      &mut self,
      iter_val: IteratorValue,
      init: A,
      mut step: F,
  ) -> EvalResult<A>
  where
      F: FnMut(&mut Self, A, Value) -> EvalResult<ControlFlow<A, A>>,
  ```
- [ ] Rewrite all 9 consumers using `drive_iterator()` (fold, count, find, any, all, for_each, collect, collect_set, join)- [ ] Verify each consumer still produces identical results (existing tests)

---

## 02.2 Extract Option/Result Method Handler

**File(s):** `compiler/ori_eval/src/interpreter/method_dispatch/collection_ops.rs`

Eight functions share the pattern: validate arg count, match receiver variant, call closure on inner value, wrap result. These are: `eval_option_map`, `eval_option_and_then`, `eval_option_filter`, `eval_option_or_else`, `eval_result_map`, `eval_result_map_err`, `eval_result_and_then`, `eval_result_or_else`.

- [ ] Create a generic handler parameterized by:
  - Which variant to match (Some/Ok/Err)
  - What to do with the inner value (call closure, call predicate)
  - How to wrap the result (Some, Ok, Err, identity)
  - **WHERE:** `compiler/ori_eval/src/interpreter/method_dispatch/collection_ops.rs` lines 379-491
- [ ] Note: the 8 handlers use inconsistent arg validation — some use `wrong_arg_count()`, some use `Self::expect_arg_count()`. Unify to a single pattern during extraction.- [ ] Rewrite the 8 functions using the shared handler
- [ ] Verify all existing tests pass unchanged

---

## 02.3 Unify Iterator Method Name Lists

**File(s):** `compiler/ori_eval/src/methods/dispatch_check.rs` (line 66: `is_collection_dispatched()`), `compiler/ori_eval/src/interpreter/resolvers/collection/mod.rs` (line ~110: `resolve_iterator_method()` if-chain), `compiler/ori_eval/src/interpreter/resolvers/mod.rs` (line 213: `all_iterator_variants()`)
Three independent lists enumerate iterator methods: `CollectionMethod::all_iterator_variants()` (in `resolvers/mod.rs`), `CollectionMethodResolver::resolve_iterator_method()` (if-chain in `resolvers/collection/mod.rs`), and `dispatch_check::is_collection_dispatched()` (in `methods/dispatch_check.rs`). Note: there are already enforcement tests in `resolvers/tests.rs` that validate `all_iterator_variants()` against `is_iterator_method()` — the goal here is to extend this pattern to cover `dispatch_check` too, and ideally derive the routing from the canonical list.
- [ ] Make `resolve_iterator_method()` derive its routing from `all_iterator_variants()` — use a lookup map built from the canonical list
- [ ] Make `is_collection_dispatched()` derive from `all_iterator_variants()` — check membership in the same lookup map
- [ ] Add an enforcement test: `all_iterator_variants().iter().all(|(name, _)| is_collection_dispatched(name))`
- [ ] Verify: adding a new iterator method to `all_iterator_variants()` is the ONLY change needed (routing and dispatch check are derived)

---

## 02.4 Eliminate Redundant Name Interning

**File(s):** `compiler/ori_eval/src/interpreter/derived_methods.rs`, `compiler/ori_eval/src/interpreter/interned_names.rs`, `compiler/ori_eval/src/methods/names.rs`

- [ ] Merge overlapping entries between `OpNames` and `BuiltinMethodNames` — shared names ("add", "compare", "bit_and", etc.) should be interned once
- [ ] Replace `self.interner.intern("to_str")` in `format_value_printable()` (line 376 of `derived_methods.rs`) with `self.builtin_method_names.to_str` (already available — see `consumers.rs:202` which uses it correctly)- [ ] Replace `self.interner.intern("default")` in `eval_default_construct()` (lines 446, 484 of `derived_methods.rs`) with a pre-interned name — add `default` to `BuiltinMethodNames` in `interned_names.rs` if not already present- [ ] Verify no runtime interning of known-at-startup method names remains in `derived_methods.rs`: `grep -n 'interner\.intern\|\.intern(' compiler/ori_eval/src/interpreter/derived_methods.rs` returns 0 matches
---

### Cleanup (fix while touching these files)
- [ ] **[WASTE]** `compiler/ori_eval/src/interpreter/method_dispatch/collection_ops.rs` — 8 Option/Result handlers use `unreachable!("...")` without structured context; replace with `unreachable!("eval_option_map dispatched on {receiver:?}")` to aid debugging on panic
- [ ] **[WASTE]** `compiler/ori_eval/src/interpreter/method_dispatch/collection_ops.rs:130-136` — `then_with` arg check uses inline `EvalError::new(format!(...))` instead of `wrong_arg_count("then_with", 1, args.len())` error factory; inconsistent with rest of file

---

## 02.R Third Party Review Findings

- None.

---

## 02.T Test Strategy

This section is pure structural refactoring with zero behavioral change. The test strategy focuses on:
1. **Existing test suite as regression gate:** `./test-all.sh` must pass identically before and after each sub-section.
2. **Unit tests for new canonical functions:** Each extracted function gets direct unit tests.
3. **Enforcement tests for method name canonical source:** Verify all consumers derive from the single list.

- [ ] Add unit tests for `drive_iterator()` in `compiler/ori_eval/src/interpreter/method_dispatch/iterator/tests.rs`:
  - Empty iterator produces init value unchanged
  - `ControlFlow::Break` stops iteration early and returns the break value
  - `ControlFlow::Continue` processes all elements
  - Test with a 3-element iterator to verify ordering (first, second, third)
- [ ] Add unit tests for the Option/Result shared handler:
  - `Some(v)` with identity closure returns `Some(v)`
  - `None` with any closure returns `None`
  - `Ok(v)` maps correctly; `Err(e)` passes through unchanged (and vice versa for `map_err`)
- [ ] Add enforcement test: `all_iterator_variants()` entries are all present in `is_collection_dispatched()`
- [ ] Verify `timeout 150 cargo test -p ori_eval` passes after each sub-section
- [ ] Verify `timeout 150 ./test-all.sh` passes after all sub-sections complete

---

## 02.N Completion Checklist

- [ ] `drive_iterator()` extracted — 9 consumers use it
- [ ] Option/Result handler extracted — 8 functions use it
- [ ] Iterator method name lists reduced to 1 canonical source
- [ ] No runtime interning of known method names in derived_methods.rs
- [ ] Unit tests for all new canonical functions pass
- [ ] Enforcement test for iterator method name derivation passes
- [ ] `timeout 150 cargo test -p ori_eval` passes
- [ ] `timeout 150 ./test-all.sh` passes
- [ ] `./clippy-all.sh` clean
- [ ] `/tpr-review` covering Section 02
- [ ] `/impl-hygiene-review last commit`
