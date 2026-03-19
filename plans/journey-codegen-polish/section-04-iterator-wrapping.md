---
section: "04"
title: "Iterator Option Wrapping Overhead"
status: not-started
reviewed: false
goal: "Eliminate unnecessary option struct wrapping and alloca round-trip in for-loop iterator codegen"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "04.1"
    title: "Direct has-next check without option wrapping"
    status: not-started
  - id: "04.2"
    title: "Eliminate element alloca round-trip"
    status: not-started
  - id: "04.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "04.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 04: Iterator Option Wrapping Overhead

**Status:** Not Started
**Goal:** Reduce the per-iteration overhead of for-loop iterator codegen by eliminating the option struct wrapping (`build_struct` of `{i64, T}`) and alloca round-trip pattern (store element to alloca, then load from alloca to pass by pointer).

**Context:** J15's `@count_chars` iterates over `[str]` with `for w in words`. The iterator's `next()` returns via `ori_iter_next(iter, scratch, elem_size) -> i8` (has_next flag). The codegen then: (1) wraps the has-next flag and element into an `{i64, {i64, i64, ptr}}` option struct, (2) extracts the tag to branch on, (3) extracts the element into a separate alloca, (4) loads from that alloca to pass to `ori_str_len`. Steps 1, 3, and 4 are unnecessary — the has-next flag can be checked directly and the element can be used from the scratch buffer.

**Depends on:** None.

---

## 04.1 Direct has-next check without option wrapping

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/builtins/iterator.rs` (lines 169-221), `compiler/ori_arc/src/lower/control_flow/for_loops/for_iterator.rs` (lines 91-121)

The ARC IR lowering in `for_iterator.rs` calls `__iter_next(iter)` (line 102-107) which returns a `{tag, element}` struct. The tag is extracted via `Project(next_result, 0)` (line 110) and the element via `Project(next_result, 1)` (line 129). The LLVM emitter `emit_iter_next()` in `iterator.rs` (lines 176-221) translates this by: calling `ori_iter_next(iter, scratch, elem_size) -> i8`, zero-extending to i64, loading the element from the scratch buffer, and building a `{i64, T}` struct. The for-loop codegen then projects field 0 (tag) and field 1 (element) from this struct.

- [ ] Investigate whether the for-loop ARC IR representation can use the `i8` has_next flag directly without wrapping into an option struct
  - The ARC IR `Project(0)` and `Project(1)` instructions reference the `{tag, elem}` struct. Eliminating the struct requires changing how the for-loop is lowered in `for_iterator.rs`
  - Alternative: keep the ARC IR representation but optimize the LLVM emission to avoid the struct materialization when the element is only used by pointer-forwarded runtime calls
  - **ARC IR feasibility analysis**: In `for_iterator.rs:102-107`, the `__iter_next` call returns a single ArcVarId (`next_result`) with type `Idx::INT` (line 103 — INT is used to suppress ARC RC on the wrapper struct). The tag is extracted via `Project(next_result, 0)` at line 110, and the element via `Project(next_result, 1)` at lines 129 and 151. Changing this to two separate return values (tag + elem) would require either: (a) multiple return values from `emit_apply`, which ARC IR does not support, or (b) using two separate `emit_apply` calls, which would change the iteration protocol. The ARC IR approach is NOT feasible without significant IR changes.
  - **Recommended**: Keep the ARC IR representation. Optimize at the LLVM emission level in `emit_iter_next()` (iterator.rs:176-221).

- [ ] **LLVM emission optimization**: In `emit_iter_next()` (iterator.rs:176-221), instead of building a `{i64, T}` struct via `build_struct()` (line 219), return a representation that the `Project` emission can decompose:
  - When ARC IR does `Project(next_result, 0)` → return the `tag` value directly (already an i64)
  - When ARC IR does `Project(next_result, 1)` → return the scratch buffer pointer or loaded element directly
  - This can be done by storing `tag` and `elem` as separate values in the `var_map` entry, using the `EmittedValue::Struct` representation (if it exists) or a new representation variant
  - **Confirmed**: `EmittedValue` (context.rs:52-71) has `Immediate`, `RcPointer`, `Aggregate`, `Pair` (dead_code, reserved for roadmap RcStrategy split), and `ZeroSized`. There is no decomposed struct variant. Adding one (e.g., `DecomposedStruct { fields: Vec<ValueId> }`) would work but requires updating `into_raw()`, `var()`, and all `Project` emission paths to handle it. The `Pair` variant is reserved for a future roadmap item (RcStrategy split) and should NOT be repurposed. Alternative: use `def_var` with `Aggregate` for the struct as today, but teach `emit_project` to recognize when the source is an `__iter_next` result and short-circuit to the scratch buffer pointer for field 1. This is less invasive but more fragile.
  - **Complexity warning**: This optimization touches the `var_map` contract (every ArcVarId maps to exactly one EmittedValue). Adding a decomposed representation changes this contract and affects all code that calls `self.var()`. Tread carefully and add `debug_assert!`s at all `into_raw()` call sites.

- [ ] If the ARC IR change is too invasive, optimize at the LLVM emission level: when the for-loop body only uses the element by pointer (e.g., `ori_str_len`), pass the scratch buffer pointer directly instead of loading the element into an alloca

---

## 04.2 Eliminate element alloca round-trip

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/builtins/iterator.rs`

The current pattern stores the element from the option struct into a separate alloca (`str_len.self`), then loads it back to pass by pointer. The element is already available in the scratch buffer from `ori_iter_next`.

- [ ] When the element is a fat pointer type (str, [T]) that will be forwarded by pointer to a runtime function, reuse the scratch buffer pointer directly instead of copying to a new alloca
  - This requires knowing at emission time that the element will only be used by pointer — feasible since the for-loop body is known at that point
  - Careful: the scratch buffer may be overwritten by the next `ori_iter_next` call, so the element must not be used after the next iteration starts (which is guaranteed by for-loop semantics)
  - **Interaction with Section 02 (Dead Loads)**: The element alloca round-trip is closely related to the dead aggregate load problem. If the element is loaded from the scratch buffer into an SSA value, then stored to a new alloca, then the alloca pointer is forwarded to `ori_str_len` via `borrowed_param_ptrs`, the optimization is: skip the load+store entirely and forward the scratch buffer pointer directly. This is the SAME pattern as Section 02's pointer-forwarded params. Consider implementing a unified solution.
  - **Edge case — element used by value AND by pointer**: If the loop body both passes the element by pointer (e.g., `w.length()`) AND uses it as a value (e.g., `print(msg: w)`), the scratch buffer reuse is unsafe for the pointer path if the value path loads from the same scratch buffer. In practice, `print(msg: w)` also forwards by pointer, so this is unlikely. But add a `debug_assert!` or guard.
  - **Edge case — nested for-loops**: If the element is an iterator or collection that spawns a nested for-loop, the scratch buffer pointer must not alias with the inner loop's scratch buffer. Verify that each `emit_iter_next` creates its own scratch alloca (line 188-192 — yes, it creates a fresh one per call).

- [ ] Add test: `for w in words do total += w.length()` should emit no alloca for the loop element — the scratch buffer is forwarded directly to `ori_str_len`
- [ ] Add test: `for w in words do results.push(w)` where element is used as a value — must still work correctly (no scratch buffer aliasing)
- [ ] Add test: nested for-loops — outer element used in inner loop body
- [ ] **Semantic pin**: `for w in ["hello", "world"] do total += w.length()` returns correct total at runtime

---

## 04.R Third Party Review Findings

- None.

---

## 04.N Completion Checklist

- [ ] J15's `@count_chars` loop body has fewer instructions per iteration (target: eliminate option struct build, element alloca store, and element alloca load — verify exact count via IR diff)
- [ ] No alloca round-trip for for-loop elements that are only pointer-forwarded
- [ ] Elements used by value (not just pointer) still work correctly
- [ ] `timeout 150 cargo t -p ori_llvm` passes (debug)
- [ ] `timeout 150 cargo b --release && timeout 150 cargo t -p ori_llvm --release` passes (release)
- [ ] `timeout 150 ./test-all.sh` green
- [ ] Iterator tests in `tests/spec/traits/iterator/` still pass
- [ ] Valgrind clean on iterator programs: `timeout 150 diagnostics/valgrind-aot.sh plans/code-journeys/15-fat-nested-collections.ori`
- [ ] Valgrind clean on additional iterator patterns: nested for-loops, for-loop with break, for-loop with guard

**Exit Criteria:** `ORI_DUMP_AFTER_LLVM=1 ori build plans/code-journeys/15-fat-nested-collections.ori` shows `@count_chars` with 0 unjustified instructions in the loop body. J15 scores 10.0/10.
