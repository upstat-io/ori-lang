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
**Goal:** Extract shared control-flow skeletons into canonical helpers. Eliminate the 9-way iterator consumer duplication, 8-way Option/Result handler duplication, and 3-way iterator method name list duplication. <!-- reviewed: accuracy fix — 9 not 7 -->

**Context:** The evaluator has the most algorithmic duplication of any crate. Nine iterator consumers share an identical `loop { eval_iter_next → match Some/None }` skeleton. Eight Option/Result method handlers share an identical `validate → extract → call closure → wrap` skeleton. Iterator method names are maintained in 3 independent lists that must be manually kept in sync. <!-- reviewed: accuracy fix — 9 consumers, not 7 -->

---

## 02.1 Extract Iterator Consumer Drive Function

**File(s):** `compiler/ori_eval/src/interpreter/method_dispatch/iterator/consumers.rs`

Nine functions share identical loop harness: `eval_iter_fold`, `eval_iter_count`, `eval_iter_find`, `eval_iter_any`, `eval_iter_all`, `eval_iter_for_each`, `eval_iter_collect`, `eval_iter_collect_set`, `eval_iter_join`. <!-- reviewed: accuracy fix — 9 consumers, not 7; collect_set and join were missed -->

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
- [ ] Rewrite all 9 consumers using `drive_iterator()` (fold, count, find, any, all, for_each, collect, collect_set, join) <!-- reviewed: accuracy fix -->
- [ ] Verify each consumer still produces identical results (existing tests)

---

## 02.2 Extract Option/Result Method Handler

**File(s):** `compiler/ori_eval/src/interpreter/method_dispatch/collection_ops.rs`

Eight functions share the pattern: validate arg count, match receiver variant, call closure on inner value, wrap result. These are: `eval_option_map`, `eval_option_and_then`, `eval_option_filter`, `eval_option_or_else`, `eval_result_map`, `eval_result_map_err`, `eval_result_and_then`, `eval_result_or_else`.

- [ ] Create a generic handler parameterized by:
  - Which variant to match (Some/Ok/Err)
  - What to do with the inner value (call closure, call predicate)
  - How to wrap the result (Some, Ok, Err, identity)
- [ ] Rewrite the 8 functions using the shared handler
- [ ] Verify all existing tests pass unchanged

---

## 02.3 Unify Iterator Method Name Lists

**File(s):** `compiler/ori_eval/src/methods/dispatch_check.rs`, `compiler/ori_eval/src/interpreter/resolvers/collection/mod.rs`

Three independent lists enumerate iterator methods: `CollectionMethod::all_iterator_variants()`, `CollectionMethodResolver::resolve_iterator_method()` (25-arm if-chain), and `dispatch_check::is_collection_dispatched()`.

- [ ] Make `resolve_iterator_method()` derive its routing from `all_iterator_variants()` — use a lookup map built from the canonical list
- [ ] Make `is_collection_dispatched()` derive from `all_iterator_variants()` — check membership in the same lookup map
- [ ] Add an enforcement test: `all_iterator_variants().iter().all(|(name, _)| is_collection_dispatched(name))`
- [ ] Verify: adding a new iterator method to `all_iterator_variants()` is the ONLY change needed (routing and dispatch check are derived)

---

## 02.4 Eliminate Redundant Name Interning

**File(s):** `compiler/ori_eval/src/interpreter/derived_methods.rs`, `compiler/ori_eval/src/interpreter/interned_names.rs`, `compiler/ori_eval/src/methods/names.rs`

- [ ] Merge overlapping entries between `OpNames` and `BuiltinMethodNames` — shared names ("add", "compare", "bit_and", etc.) should be interned once
- [ ] Replace `self.interner.intern("to_str")` in `format_value_printable()` (line 376) with `self.builtin_names.to_str`
- [ ] Replace `self.interner.intern("default")` in `eval_default_construct()` (lines 446, 484) with a pre-interned name
- [ ] Verify no runtime interning of known-at-startup method names remains in derived_methods.rs

---

## 02.R Third Party Review Findings

- None.

---

## 02.N Completion Checklist

- [ ] `drive_iterator()` extracted — 9 consumers use it <!-- reviewed: accuracy fix -->
- [ ] Option/Result handler extracted — 8 functions use it
- [ ] Iterator method name lists reduced to 1 canonical source
- [ ] No runtime interning of known method names in derived_methods.rs
- [ ] `timeout 150 cargo test -p ori_eval` passes
- [ ] `timeout 150 ./test-all.sh` passes
- [ ] `./clippy-all.sh` clean
- [ ] `/tpr-review` covering Section 02
- [ ] `/impl-hygiene-review last commit`
