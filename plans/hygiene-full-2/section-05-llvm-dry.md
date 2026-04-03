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
**Goal:** Eliminate algorithmic duplication within ori_llvm: 24 identical iterator dispatch stanzas, COW list mutation protocol duplication, 75-entry trait method Cartesian product, manually mirrored JIT runtime mappings, and duplicate debug helpers. <!-- reviewed: accuracy fix — 24 stanzas not 23, 75 trait entries not 82 -->

**Context:** The LLVM codegen backend has significant internal duplication, particularly in the `declare_builtins!` macro invocations and runtime mapping code. The iterator dispatch has 24 stanzas that all check `TypeInfo::Iterator { element }` and delegate to `emitter.emit_iterator_method()`. The COW list mutations repeat a multi-step protocol. The `runtime_mappings.rs` file has a TODO acknowledging it manually mirrors `RT_FUNCTIONS`. <!-- reviewed: accuracy fix — 24 stanzas not 23 -->

---

## 05.1 Consolidate Iterator Builtin Dispatch

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/builtins/iterator.rs`

24 iterator method dispatch stanzas all follow the same pattern: check `TypeInfo::Iterator`, delegate to `emitter.emit_iterator_method()`. <!-- reviewed: accuracy fix — 24 not 23 -->

- [ ] Replace the 24 individual `declare_builtins!` entries with a single `("Iterator", _)` catch-all handler that checks if the method name is a known iterator method
- [ ] Use the existing `is_iterator_method()` function (which queries `ori_registry`) to validate method names
- [ ] Keep the `declare_builtins!` registration entries for discoverability, but make them point to the single handler
- [ ] Verify: all iterator method tests pass unchanged

---

## 05.2 Extract COW List Mutation Helper

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/builtins/collections/mod.rs`, `compiler/ori_llvm/src/codegen/arc_emitter/builtins/collections/list_cow.rs` <!-- reviewed: accuracy fix — list_cow.rs is where the shared protocol lives -->

9 COW list mutation functions in `list_cow.rs` (push, pop, set, insert, remove, concat, reverse, sort, sort_stable) share the same protocol: extract element type, get cow_mode, call the runtime function, conditionally call `mark_cow_data_noalias_if_unique`. The dispatch stanzas in `collections/mod.rs` also repeat the same `TypeInfo::List { element }` extraction pattern. <!-- reviewed: accuracy fix — 9 functions, not 16 -->

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

The `declare_builtins!` in traits.rs lists ~75 entries all calling `emitter.emit_trait_method()` with identical arguments. The primitives.rs file has ~34 entries calling `emitter.emit_primitive_method()` identically. <!-- reviewed: accuracy fix — 75 not 82, 34 not 39 -->

- [ ] Generate the Cartesian product programmatically from two lists: types and trait methods
- [ ] Keep the explicit registration (needed for the `declare_builtins!` macro's map construction) but generate it from arrays/slices
- [ ] For primitives.rs: same approach — generate from `PRIMITIVE_TYPES` x `PRIMITIVE_METHODS` arrays

---

## 05.4 Generate JIT Runtime Mappings from RT_FUNCTIONS

**File(s):** `compiler/ori_llvm/src/evaluator/runtime_mappings.rs`, `compiler/ori_llvm/src/codegen/runtime_decl/runtime_functions.rs`

The `lookup_jit_address()` function (260 lines) manually mirrors the `RT_FUNCTIONS` table. The file has a TODO at line 65: "Generate this mapping from RT_FUNCTIONS data." The struct is `RtFn` (not `RuntimeFunction`), and it already has a `jit_allowed: bool` field — `jit_symbol_mappings()` already filters by it. The duplication is purely in `lookup_jit_address()` which maintains a manual match-arm-per-function mapping. <!-- reviewed: accuracy fix — RtFn not RuntimeFunction, jit_allowed already exists -->

<!-- reviewed: feasibility fix — function pointers (`fn as *const () as usize`) are NOT const-evaluable in stable Rust, so storing them directly in the static `RT_FUNCTIONS` array is not possible. The approach must use runtime initialization. -->

- [ ] Create a `LazyLock<HashMap<&'static str, usize>>` that iterates `RT_FUNCTIONS` (filtering by `jit_allowed: true`) and resolves each name to its function pointer at first access. Use a module-level `fn resolve_fn_ptr(name: &str) -> usize` with the existing match arms, or use a registration macro that pairs each `RtFn` entry with its function pointer at initialization time.
- [ ] Alternatively, use a `build.rs` or proc macro to generate the match statement from the `RT_FUNCTIONS` data, keeping the match but auto-generating it from the canonical source.
- [ ] Replace `lookup_jit_address()` body with a HashMap lookup from the lazy-initialized mapping.
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
