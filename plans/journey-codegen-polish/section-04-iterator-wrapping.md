---
section: "04"
title: "Iterator Option Wrapping Overhead"
status: complete
reviewed: true
goal: "Eliminate unnecessary option struct wrapping and alloca round-trip in for-loop iterator codegen"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "04.1"
    title: "Direct has-next check without option wrapping"
    status: complete
  - id: "04.2"
    title: "Eliminate element alloca round-trip"
    status: complete
  - id: "04.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "04.N"
    title: "Completion Checklist"
    status: complete
---

# Section 04: Iterator Option Wrapping Overhead

**Status:** Not Started
**Goal:** Reduce the per-iteration overhead of for-loop iterator codegen by eliminating the option struct wrapping (`build_struct` of `{i64, T}`) and alloca round-trip pattern (store element to alloca, then load from alloca to pass by pointer).

**Context:** J15's `@count_chars` iterates over `[str]` with `for w in words`. The iterator's `next()` returns via `ori_iter_next(iter, scratch, elem_size) -> i8` (has_next flag). The codegen then: (1) wraps the has-next flag and element into an `{i64, {i64, i64, ptr}}` option struct, (2) extracts the tag to branch on, (3) extracts the element into a separate alloca, (4) loads from that alloca to pass to `ori_str_len`. Steps 1, 3, and 4 are unnecessary — the has-next flag can be checked directly and the element can be used from the scratch buffer.

**Depends on:** None. (Soft dependency on Section 02 resolved — Section 02 is COMPLETE as of 2026-03-19.)

**Correctness Risks:**
1. **Scratch buffer lifetime**: The scratch buffer is overwritten by each `ori_iter_next` call. If the element is referenced after the next iteration starts (e.g., captured by a closure or stored in a collection), the value is stale. For-loop semantics guarantee single-iteration scope, but `break` with a captured element or `yield` with a reference could violate this if not handled.
2. **Pointer aliasing**: Registering the scratch buffer in `borrowed_param_ptrs` means downstream code may forward the pointer instead of copying the value. If any codepath reads the scratch buffer after `ori_iter_next` is called again, the data is corrupt. This is safe for for-do loops (single iteration scope), but must be verified for for-yield and for-guard paths.
3. **EmittedValue type mismatch**: `next_result` has ARC type `Idx::INT` (to suppress RC), so `def_var_repr` classifies it as `Immediate`. But the LLVM value is actually a struct `{i64, T}`. The `var()` call returns the `ValueId` of the struct regardless of the `EmittedValue` wrapper, so `extract_value` still works. Any new variant or special handling must preserve this quirk.

---

## 04.1 Direct has-next check without option wrapping

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/builtins/iterator.rs` (lines 176-221), `compiler/ori_arc/src/lower/control_flow/for_loops/for_iterator.rs` (lines 91-121), `compiler/ori_arc/src/lower/control_flow/for_yield.rs` (lines 245-268 for `__iter_next` call, 276+296 for element `Project`)

The ARC IR lowering in `for_iterator.rs` calls `__iter_next(iter)` (line 102-107) which returns a single ArcVarId (`next_result`) with type `Idx::INT` (line 103 — INT is used to suppress ARC RC on the wrapper struct). The tag is extracted via `Project(next_result, 0)` (line 110) and the element via `Project(next_result, 1)` (lines 129 and 151 — guarded block and body block respectively). The LLVM emitter `emit_iter_next()` in `iterator.rs` (lines 176-221) translates this by: calling `ori_iter_next(iter, scratch, elem_size) -> i8`, zero-extending to i64, loading the element from the scratch buffer, and building a `{i64, T}` struct. The for-loop codegen then projects field 0 (tag) and field 1 (element) from this struct.

**IMPORTANT — for-yield uses the same pattern**: `lower_for_yield_iterator()` in `compiler/ori_arc/src/lower/control_flow/for_yield.rs:151` (note: NOT in `for_loops/` subdirectory) uses the identical `__iter_next -> Project(0) -> Project(1)` pattern as `lower_for_iterator()`. Any optimization to `emit_iter_next` or `emit_project` will affect both for-do and for-yield loops. All tests must cover BOTH paths.

- [x] Investigate whether the for-loop ARC IR representation can use the `i8` has_next flag directly without wrapping into an option struct (2026-03-19)
  - The ARC IR `Project(0)` and `Project(1)` instructions reference the `{tag, elem}` struct. Eliminating the struct requires changing how the for-loop is lowered in `for_iterator.rs`
  - Alternative: keep the ARC IR representation but optimize the LLVM emission to avoid the struct materialization when the element is only used by pointer-forwarded runtime calls
  - **ARC IR feasibility analysis**: In `for_iterator.rs:102-107`, the `__iter_next` call returns a single ArcVarId (`next_result`) with type `Idx::INT` (line 103 — INT is used to suppress ARC RC on the wrapper struct). The tag is extracted via `Project(next_result, 0)` at line 110, and the element via `Project(next_result, 1)` at lines 129 and 151. Changing this to two separate return values (tag + elem) would require either: (a) multiple return values from `emit_apply`, which ARC IR does not support, or (b) using two separate `emit_apply` calls, which would change the iteration protocol. The ARC IR approach is NOT feasible without significant IR changes.
  - **Recommended**: Keep the ARC IR representation. Optimize at the LLVM emission level in `emit_iter_next()` (iterator.rs:176-221).

- [x] **LLVM emission optimization**: In `emit_iter_next()` (iterator.rs:176-221), instead of building a `{i64, T}` struct via `build_struct()` (line 219), return a representation that the `Project` emission can decompose: (2026-03-19 — Approach A implemented)
  - When ARC IR does `Project(next_result, 0)` -> return the `tag` value directly (already an i64)
  - When ARC IR does `Project(next_result, 1)` -> return the scratch buffer pointer or loaded element directly
  - This can be done by storing `tag` and `elem` as separate values in the `var_map` entry, using the `EmittedValue::Struct` representation (if it exists) or a new representation variant
  - **Confirmed**: `EmittedValue` (context.rs:128-147) has `Immediate`, `RcPointer`, `Aggregate`, `Pair` (reserved for roadmap RcStrategy split), and `ZeroSized`. There is no decomposed struct variant. Adding one (e.g., `DecomposedStruct { fields: Vec<ValueId> }`) would work but requires updating `into_raw()`, `var()`, and all `Project` emission paths to handle it. The `Pair` variant is reserved for a future roadmap item (RcStrategy split) and should NOT be repurposed. Alternative: use `def_var` with `Aggregate` for the struct as today, but teach `emit_project` to recognize when the source is an `__iter_next` result and short-circuit to the scratch buffer pointer for field 1. This is less invasive but more fragile.
  - **Complexity warning**: This optimization touches the `var_map` contract (every ArcVarId maps to exactly one EmittedValue). Adding a decomposed representation changes this contract and affects all code that calls `self.var()`. Tread carefully and add `debug_assert!`s at all `into_raw()` call sites.

- [x] **Choose and implement one of these concrete approaches** (do NOT defer): (2026-03-19 — Approach A selected and implemented)
  - **Approach A — Side-channel map** (recommended): Add a `iter_next_decomposed: FxHashMap<ArcVarId, (ValueId, ValueId)>` field to `ArcIrEmitter` (in `mod.rs`). In `emit_iter_next`, instead of `build_struct`, store `(tag, scratch_ptr)` in this map keyed by `dst`. In `emit_project` (instr_dispatch.rs:53-179), before the existing logic, check if `value` is in `iter_next_decomposed`: if field==0 return tag, if field==1 load from scratch_ptr and also register scratch_ptr in `borrowed_param_ptrs` for `dst`. This avoids any changes to `EmittedValue` or the `var_map` contract. Still requires a dummy value in `var_map` (e.g., `Immediate(tag)` — the tag alone, not the struct).
    - **Sync points**: `mod.rs` (new field + initialization at line 185-222), `iterator.rs` (emit_iter_next), `instr_dispatch.rs` (emit_project), `emitter_utils.rs` (verify `var()` still works for decomposed vars — it will return the tag, which is fine because only `Project` accesses the struct)
    - **Risk**: Low — no `EmittedValue` changes, no `into_raw()` changes, no `var_map` contract changes. Side-channel is scoped to a single optimization.
  - **Approach B — New EmittedValue variant**: Add `Decomposed { tag: ValueId, scratch_ptr: ValueId }`. Requires updating `into_raw()` (panic like `Pair`), `var_emitted()` callers in RC ops, `from_repr()`. Higher risk, wider blast radius, but cleaner type-level guarantee that the value is decomposed.
    - **Sync points for EmittedValue changes**: `context.rs` (variant + `into_raw` + `rc_data_ptr` + `from_repr`), `emitter_utils.rs` (`var()` must handle or panic), ALL RC emission in `rc_ops.rs` / `instr_dispatch.rs` (any `var_emitted()` call), `tests.rs` (unit tests for `into_raw` panic on Decomposed).
  - **Approach C — Pattern-match in emit_project** (fragile): Keep current `build_struct`, but teach `emit_project` to recognize `{i64, T}` structs from `emit_iter_next` and avoid the alloca round-trip when forwarding. Requires a way to identify which values came from `emit_iter_next` (e.g., a `HashSet<ArcVarId>`). Does NOT eliminate the struct building, only the alloca — partial win.

- [x] **Fallback** (secondary — not needed, Approach A worked): Optimize at the LLVM emission level by passing the scratch buffer pointer directly when the for-loop body only uses the element by pointer (e.g., `ori_str_len`). Subsumed by Approach A. (2026-03-19)

- [x] **IR diff verification**: Before and after the optimization, dump LLVM IR for J15's `@count_chars`. Verified: (2026-03-19)
  - `insertvalue` instructions eliminated from iter_next path (2 removed from bb1)
  - `extractvalue` on the `{i64, T}` struct eliminated (2 removed: 1 from bb1, 1 from bb2)
  - `str_len.self` alloca eliminated from bb0
  - `store` to alloca eliminated from bb2
  - `ori_str_len` now receives `%iter_next.scratch` directly
  - bb1 reduced from 8 to 4 instructions (call, zext, icmp, br)
  - Element load deferred from bb1 to bb2 (only when has_next=1)

---

## 04.2 Eliminate element alloca round-trip

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/builtins/iterator.rs`, `compiler/ori_llvm/src/codegen/arc_emitter/instr_dispatch.rs` (emit_project, lines 53-179), `compiler/ori_llvm/src/codegen/arc_emitter/apply_helpers.rs` (apply_param_passing_with_forwarding), `compiler/ori_llvm/src/codegen/arc_emitter/builtins/collections/string_builtins.rs` (str_to_ptr_forwarded callsite at line 40), `compiler/ori_llvm/src/codegen/arc_emitter/builtins/collections/mod.rs` (str_to_ptr_forwarded definition at line 502)

The current pattern stores the element from the option struct into a separate alloca (`str_len.self`), then loads it back to pass by pointer. The element is already available in the scratch buffer from `ori_iter_next`.

- [x] When the element is a fat pointer type (str, [T]) that will be forwarded by pointer to a runtime function, reuse the scratch buffer pointer directly instead of copying to a new alloca (2026-03-19 — `emit_project_iter_next` registers scratch_ptr in `borrowed_param_ptrs`, enabling automatic forwarding via existing infrastructure)
  - This requires knowing at emission time that the element will only be used by pointer — feasible since the for-loop body is known at that point
  - Careful: the scratch buffer may be overwritten by the next `ori_iter_next` call, so the element must not be used after the next iteration starts (which is guaranteed by for-loop semantics)
  - **Key finding**: `borrowed_param_ptrs` already propagates through `Let` aliases (instr_dispatch.rs:195-198 — the `Let { dst, Var(src) }` arm copies the source's `borrowed_param_ptrs` entry to the destination). If the scratch buffer pointer is registered as the `borrowed_param_ptrs` entry for the element's ArcVarId (the `Project(next_result, 1)` destination), existing forwarding infrastructure in `apply_param_passing_with_forwarding` and `str_to_ptr_forwarded` would automatically forward the scratch pointer to runtime calls like `ori_str_len` — no new forwarding mechanism needed.
  - **Interaction with Section 02 (Dead Loads)** [COMPLETE]: The element alloca round-trip is related to the dead aggregate load problem. Section 02 created `compute_pointer_only_params()` in `field_scan/mod.rs` with an `is_forwarding_safe()` callback pattern — it identifies *function parameters* whose loaded values are never needed because all callees use `borrowed_param_ptrs` pointer forwarding. The iterator scratch buffer is NOT a function parameter, so `compute_pointer_only_params()` does not directly apply. However, the underlying pattern is the same: the scratch buffer pointer could be forwarded to `ori_str_len` via `borrowed_param_ptrs` instead of loading the element into a new alloca. The `borrowed_param_ptrs` map itself is pre-existing infrastructure (not created by Section 02). Consider whether the scratch buffer pointer can be registered in `borrowed_param_ptrs` for the loop element variable, enabling existing pointer forwarding to handle it.
  - **Edge case — element used by value AND by pointer**: If the loop body both passes the element by pointer (e.g., `w.length()`) AND uses it as a value (e.g., `print(msg: w)`), the scratch buffer reuse is unsafe for the pointer path if the value path loads from the same scratch buffer. In practice, `print(msg: w)` also forwards by pointer, so this is unlikely. But add a `debug_assert!` or guard.
  - **Edge case — nested for-loops**: If the element is an iterator or collection that spawns a nested for-loop, the scratch buffer pointer must not alias with the inner loop's scratch buffer. Verify that each `emit_iter_next` creates its own scratch alloca (iterator.rs:188-192 — yes, it creates a fresh one per call).

- [x] **Edge case — element captured by closure**: (2026-03-19 — verified PartialApply does not reference `borrowed_param_ptrs`) If the loop body captures the element in a closure (e.g., `for w in words do callbacks.push(() -> w.length())`), the closure captures by value. The captured value must be a copy, not the scratch buffer pointer. Verify that closure capture (PartialApply) loads the element value, not the pointer. If it reads `borrowed_param_ptrs`, the captured pointer would be stale after the iteration. **Guard**: `borrowed_param_ptrs` forwarding should NOT apply to PartialApply args — verify this in `instr_dispatch.rs` (PartialApply emission at lines 217-222). Codebase scan confirmed that the PartialApply emission path does not reference `borrowed_param_ptrs`, so PartialApply is safe by default. Still verify this holds after implementation.
- [x] **Edge case — element stored into collection**: (2026-03-19 — `ori_list_push` copies immediately, scratch forwarding is safe) If the loop body pushes the element into a list (e.g., `for w in words do results.push(w)`), the runtime copies the element by value. The `borrowed_param_ptrs` pointer should not be used for `ori_list_push`'s element parameter — `ori_list_push` takes an element pointer and copies `elem_size` bytes. Forwarding the scratch buffer pointer here is actually SAFE because `ori_list_push` copies immediately. But verify this assumption.
- [x] **Edge case — for-yield (list comprehension)**: (2026-03-19 — for-yield uses identical `__iter_next -> Project` pattern, optimization applies automatically) `for w in words yield w.length()` uses `lower_for_yield_iterator()` in `compiler/ori_arc/src/lower/control_flow/for_yield.rs` (NOT `for_loops/for_yield.rs`) which emits `ori_list_push(list_ptr, elem_ptr, elem_size)` for each yielded value. The yielded value is the BODY result (e.g., `w.length()` returns `int`), not the iterator element directly. However, the element `w` still comes from `Project(next_result, 1)` (for_yield.rs:276 in guard block, :296 in body block) and could be forwarded. Verify the for-yield path works with scratch buffer forwarding.
- [x] **Edge case — guarded for-loop with element binding**: (2026-03-19 — both guard and body Projects load from same scratch pointer, correct since buffer isn't overwritten between them) `for w in words if w.length() > 3 do total += w.length()` binds the element in the guard block (for_iterator.rs:129-130) AND the body block (for_iterator.rs:151-152). Both `Project(next_result, 1)` destinations must get the correct forwarding. With Approach A (side-channel), both Projects would load from the same scratch pointer, which is correct since the scratch buffer hasn't been overwritten between guard and body.

- [x] **Verify borrowed_param_ptrs propagation chain**: (2026-03-19 — verified via IR dump: `ori_str_len(ptr %iter_next.scratch)` confirms end-to-end forwarding) Trace the full path from scratch buffer registration to runtime call forwarding:
  1. `emit_iter_next` stores `scratch` ValueId in side-channel (or `borrowed_param_ptrs`)
  2. `emit_project` for field 1 registers scratch pointer in `borrowed_param_ptrs` for the `dst` ArcVarId
  3. `Let { dst2, Var(dst) }` in `instr_dispatch.rs:195-198` propagates `borrowed_param_ptrs[dst]` to `dst2`
  4. `str_to_ptr_forwarded` in `string_builtins.rs:40` checks `borrowed_param_ptrs[var]` -> returns scratch pointer directly
  5. `apply_param_passing_with_forwarding` in `apply_helpers.rs:96-99` checks `borrowed_param_ptrs[arc_var]` -> forwards pointer
  - Write a test that traces this chain end-to-end for both `str.length()` and `ori_str_len` paths

### Test Matrix

**Element type axis:**
- [x] Add test: `for w in words do total += w.length()` where `words: [str]` — `test_iter_for_loop_str_length_correctness` + `test_iter_next_no_wrapper_struct` (2026-03-19)
- [x] Add test: `for xs in nested do total += xs.length()` where `nested: [[int]]` — `test_iter_for_loop_list_length_correctness` (2026-03-19)
- [x] Add test: `for x in items do total += x` where `items: [int]` — `test_iter_for_loop_scalar_element` (2026-03-19)
- [x] Add test: `for p in points do total += p.x` where `points: [Point]` — `test_iter_for_loop_struct_field_access` (2026-03-19)

**Control-flow pattern axis:**
- [x] Add test: `for w in words do results.push(w)` where element is stored into a collection — `test_iter_for_loop_push_element` (2026-03-19)
- [x] Add test: nested for-loops — `test_iter_nested_for_loops` (`for xs in lists do for x in xs do total += x`) (2026-03-19)
- [x] Add test: `for w in words if w.length() > 3 do total += w.length()` — `test_iter_for_loop_guarded` (2026-03-19)
- [x] Add test: `for w in words do { if w.length() == 5 then total += 1; if total == 2 then break }` — `test_iter_for_loop_with_break` (2026-03-19)
- [x] Add test: `for w in words yield w.length()` — `test_iter_for_yield_lengths` (2026-03-19)

**Negative tests (optimization must NOT be applied):**
- [x] Add test: function that takes an element by value and modifies it — `test_iter_for_loop_element_passed_to_function` (2026-03-19)
- [x] Add test: element passed to TWO different runtime calls in the same iteration — `test_iter_for_loop_two_calls_same_element` (2026-03-19)

**Semantic pins:**
- [x] **Semantic pin**: `for w in ["hello", "world"] do total += w.length()` returns correct total (10) — `test_iter_for_loop_str_length_correctness` (2026-03-19)
- [x] **Semantic pin**: `for xs in [[1,2,3], [4,5]] do total += xs.length()` returns correct total (5) — `test_iter_for_loop_list_length_correctness` (2026-03-19)
- [x] **Semantic pin**: `for w in ["a", "bb", "ccc"] yield w.length()` returns `[1, 2, 3]` — `test_iter_for_yield_semantic_pin` (2026-03-19)

---

## Cleanup

- [x] **[BLOAT check]** `iterator.rs` = 327 lines (REDUCED from 337 — removed struct building), `instr_dispatch.rs` = 430 lines (grew due to extracted methods, under 500 limit). All files under limit. (2026-03-19)
- [x] **[DRIFT check]** `iter_next_decomposed` field initialized to `FxHashMap::default()` in `ArcIrEmitter::new()` (mod.rs). Emitter is constructed per-function — no cross-function leakage. (2026-03-19)
- [x] **[WASTE fix]** `context.rs:267`: `#[allow(dead_code)]` converted to `#[expect(dead_code)]` (2026-03-19)

---

## 04.R Third Party Review Findings

- None.

---

## 04.N Completion Checklist

- [x] J15's `@count_chars` loop body has fewer instructions per iteration — bb1 reduced from 8 to 4 instructions, bb2 eliminated store+alloca (2026-03-19)
- [x] IR diff captured: before (load+2x insertvalue+2x extractvalue+store) → after (direct tag+scratch forwarding) (2026-03-19)
- [x] No alloca round-trip for for-loop elements that are only pointer-forwarded — `ori_str_len(ptr %iter_next.scratch)` confirmed (2026-03-19)
- [x] Elements used by value (not just pointer) still work correctly — 13,328 tests pass (2026-03-19)
- [x] Scalar elements (int, float, bool) are unaffected — `test_iter_for_loop_scalar_element` passes (2026-03-19)
- [x] for-yield (list comprehension) path works correctly — 4181 Ori spec tests pass including iterator tests (2026-03-19)
- [x] `timeout 150 cargo t -p ori_llvm` passes (debug) — 997 tests (2026-03-19)
- [x] `timeout 150 cargo b --release && timeout 150 cargo t -p ori_llvm --release` passes (release) — 995 tests (2026-03-19)
- [x] `timeout 150 ./test-all.sh` green — 13,328 passed, 0 failed (2026-03-19)
- [x] `timeout 150 ./clippy-all.sh` green (2026-03-19)
- [x] Iterator tests in `tests/spec/traits/iterator/` still pass (2026-03-19)
- [x] For-loop spec tests in `tests/spec/` still pass (2026-03-19)
- [x] Valgrind clean on iterator programs — `diagnose-aot.sh --valgrind` reports no memory errors (2026-03-19)
- [x] Valgrind clean on additional iterator patterns: nested for-loops, for-loop with break, for-loop with guard, for-loop with yield — 4 Valgrind tests in `tests/valgrind/iter_rc/` (2026-03-19)
- [x] No new `debug_assert!` failures in debug build — full test suite passed (2026-03-19)
- [x] `borrowed_param_ptrs` map does NOT leak across iterations — emitter is per-function, map resets each function (2026-03-19)
- [x] EmittedValue was NOT modified (Approach A used) — no `into_raw()` changes needed (2026-03-19)
- [x] `context.rs:267` `#[allow(dead_code)]` converted to `#[expect(dead_code)]` (2026-03-19)

**Exit Criteria:** `ORI_DUMP_AFTER_LLVM=1 ori build plans/code-journeys/15-fat-nested-collections.ori` shows `@count_chars` with 0 unjustified instructions in the loop body. J15 scores 10.0/10.
