---
section: "05"
title: "LLVM Codegen Internal DRY"
status: not-started
reviewed: false
goal: "Extract shared codegen patterns — iterator dispatch, COW mutation protocol, trait method Cartesian product, and JIT runtime mappings"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "05.1"
    title: "Consolidate Iterator Builtin Dispatch"
    status: not-started
  - id: "05.2"
    title: "Extract COW List Mutation Helper"
    status: not-started
  - id: "05.3"
    title: "Collapse Trait Method Cartesian Product"
    status: not-started
  - id: "05.4"
    title: "Generate JIT Runtime Mappings from RT_FUNCTIONS"
    status: not-started
  - id: "05.5"
    title: "Merge Duplicate Debug Helpers"
    status: not-started
  - id: "05.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "05.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 05: LLVM Codegen Internal DRY

**Status:** Not Started
**Goal:** Eliminate algorithmic duplication within ori_llvm: 23 identical iterator dispatch stanzas, 16 identical COW mutation entries, 82-entry trait method Cartesian product, manually mirrored JIT runtime mappings, and duplicate debug helpers.

**Context:** The LLVM codegen backend has significant internal duplication, particularly in the `declare_builtins!` macro invocations and runtime mapping code. The iterator dispatch has 23 stanzas that all check `TypeInfo::Iterator { element }` and delegate to `emitter.emit_iterator_method()`. The COW list mutations repeat a 5-step protocol 16 times. The `runtime_mappings.rs` file has a TODO acknowledging it manually mirrors `RT_FUNCTIONS`.

---

## 05.1 Consolidate Iterator Builtin Dispatch

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/builtins/iterator.rs`

23 iterator method dispatch stanzas all follow the same pattern: check `TypeInfo::Iterator`, delegate to `emitter.emit_iterator_method()`.

- [ ] Replace the 23 individual `declare_builtins!` entries with a single `("Iterator", _)` catch-all handler that checks if the method name is a known iterator method
- [ ] Use the existing `is_iterator_method()` function (which queries `ori_registry`) to validate method names
- [ ] Keep the `declare_builtins!` registration entries for discoverability, but make them point to the single handler
- [ ] Verify: all iterator method tests pass unchanged

---

## 05.2 Extract COW List Mutation Helper

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/builtins/collections/mod.rs`

16 COW list methods repeat: extract element type, get cow_mode, call `emit_list_*_cow`, conditionally call `mark_cow_data_noalias_if_unique`.

- [ ] Create `emit_cow_list_op()` higher-order function:
  ```rust
  fn emit_cow_list_op<F>(
      emitter: &mut ArcEmitter,
      ctx: &BuiltinContext,
      op: F,
  ) -> Option<BasicValueEnum>
  where F: FnOnce(&mut ArcEmitter, ...) -> BasicValueEnum
  ```
- [ ] Refactor push, pop, set, insert, remove, concat, reverse, sort, sort_stable to use `emit_cow_list_op()`
- [ ] Verify: `timeout 150 cargo test -p ori_llvm` passes

---

## 05.3 Collapse Trait Method Cartesian Product

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/builtins/traits.rs`, `compiler/ori_llvm/src/codegen/arc_emitter/builtins/primitives.rs`

The `declare_builtins!` in traits.rs lists ~82 entries (7 types x ~12 methods) all calling `emitter.emit_trait_method()` with identical arguments. The primitives.rs file has ~39 entries calling `emitter.emit_primitive_method()` identically.

- [ ] Generate the Cartesian product programmatically from two lists: types and trait methods
- [ ] Keep the explicit registration (needed for the `declare_builtins!` macro's map construction) but generate it from arrays/slices
- [ ] For primitives.rs: same approach — generate from `PRIMITIVE_TYPES` x `PRIMITIVE_METHODS` arrays

---

## 05.4 Generate JIT Runtime Mappings from RT_FUNCTIONS

**File(s):** `compiler/ori_llvm/src/evaluator/runtime_mappings.rs`, `compiler/ori_llvm/src/codegen/runtime_decl/runtime_functions.rs`

The `lookup_jit_address()` function (260 lines) manually mirrors the `RT_FUNCTIONS` table. The file has a TODO at line 66: "Generate this mapping from RT_FUNCTIONS data instead of maintaining a manual mirror."

- [ ] Add a `jit_allowed: bool` field to `RuntimeFunction` entries in `RT_FUNCTIONS` (or filter by existing criteria)
- [ ] Generate `lookup_jit_address()` by iterating `RT_FUNCTIONS` and matching on name, using the function pointer from each entry
- [ ] Delete the manual 186-line match statement
- [ ] Verify: JIT tests pass unchanged (`timeout 150 cargo test -p ori_llvm -- jit`)

---

## 05.5 Merge Duplicate Debug Helpers

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/builtins/debug_helpers.rs`

`emit_result_debug` (lines 254-312) and `emit_nested_result_debug` (lines 322-377) have the same structure: extract tag, branch on Ok/Err, format payload, merge via phi.

- [ ] Have `emit_result_debug` delegate to `emit_nested_result_debug` (or vice versa) after resolving the entry point difference
- [ ] Verify: debug formatting tests pass unchanged

---

## 05.R Third Party Review Findings

- None.

---

## 05.N Completion Checklist

- [ ] Iterator dispatch consolidated to single handler
- [ ] COW list mutation helper extracted
- [ ] Trait method Cartesian product generated programmatically
- [ ] JIT runtime mappings generated from RT_FUNCTIONS
- [ ] Duplicate debug helpers merged
- [ ] `timeout 150 cargo test -p ori_llvm` passes
- [ ] `timeout 150 ./test-all.sh` passes
- [ ] `./clippy-all.sh` clean
- [ ] `/tpr-review` covering Section 05
- [ ] `/impl-hygiene-review last commit`
