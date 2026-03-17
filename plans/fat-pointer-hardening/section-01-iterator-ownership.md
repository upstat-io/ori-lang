---
section: "01"
title: "Iterator–Collection Ownership Contract"
status: not-started
goal: "Fix the ownership contract between iterators and collections so that [T] where T has Drop never double-frees elements"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "01.1"
    title: "Root Cause Analysis"
    status: not-started
  - id: "01.2"
    title: "Fix Element Ownership Contract"
    status: not-started
  - id: "01.3"
    title: "Fix Unwind Path Double Drop"
    status: not-started
  - id: "01.4"
    title: "Generalize to All [T] Where T Has Drop"
    status: not-started
  - id: "01.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "01.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 01: Iterator–Collection Ownership Contract

**Status:** Not Started
**Goal:** When iterating over `[T]` where `T` has Drop semantics (str, [T], closures, structs with Drop fields), exactly one entity owns each element at any point. No double-frees, no leaks. This applies to ALL such types, not just `[str]`.

**Context:** J15 discovered that iterating over `[str]` causes a double-free. The iterator runtime (`ori_iter_drop`) frees each string element, AND the list destructor (`ori_buffer_rc_dec` calling `_ori_elem_dec`) also frees the same elements. This is because the ownership contract between iterators and collections was never defined for element types that themselves have RC — J10 tested `[int]` (scalar elements, no element-level RC) which masked the issue.

**Crate scope:** The fix spans 4 subsystems across 3 crates:
1. `ori_rt/src/iterator/state.rs` -- runtime `IterState::List` Drop impl
2. `ori_arc/src/lower/control_flow/for_loops/` -- ARC IR lowering for `for x in list` (creates IterState, passes `elem_dec_fn`)
3. `ori_arc/src/aims/emit_rc/` -- RC emission that decides whether to emit RcDec on iterator elements
4. `ori_llvm/src/codegen/arc_emitter/` -- LLVM codegen for the ARC IR's RC ops

The pipeline is: `ori_arc` lowers `for w in words` into ARC IR with RcDec on `w`, `ori_llvm` emits the LLVM IR for that RcDec, and `ori_rt`'s `IterState::List` Drop emits ANOTHER `buffer_rc_dec` with `elem_dec_fn`. Both paths free the same elements.

**Reference implementations:**
- **Rust** `alloc/src/vec/into_iter.rs`: `IntoIter` takes ownership of elements, sets Vec length to 0 so Vec's Drop skips them
- **Swift** `stdlib/public/core/Array.swift`: Iterators borrow elements; collection retains ownership throughout
- **Lean 4** `src/Lean/Compiler/IR/RC.lean`: Iterator consumes borrowed references; collection owns all elements

---

## 01.1 Root Cause Analysis

**File(s):** `compiler/ori_rt/src/iterator/state.rs`, `compiler/ori_rt/src/rc/list_rc.rs`

The double-free happens because two independent cleanup paths both try to free the same elements:

1. **Iterator path**: When the iterator is dropped (`IterState::List` Drop impl), it calls element-level RC decrement on remaining un-consumed elements
2. **Collection path**: When the list buffer's RC reaches zero, `ori_buffer_rc_dec` calls the element destructor (`_ori_elem_dec$N`) on ALL elements, including ones already freed by the iterator

The fundamental issue: **there is no handoff of element ownership from collection to iterator.** Both think they own the elements.

- [ ] Trace the full lifecycle of a `[str]` iteration in AOT with `ORI_TRACE_RC=1` to confirm the double-free sequence
- [ ] Identify exactly which RC operations fire on each string element during iteration and after
- [ ] Document the current ownership model: who increments, who decrements, at what points
- [ ] Trace how `elem_dec_fn` is generated: `ori_arc/src/drop/mod.rs` computes `DropInfo` for `[str]`, `ori_llvm/src/codegen/arc_emitter/element_fn_gen.rs` generates the LLVM function, and `ori_llvm/src/codegen/arc_emitter/construction.rs` passes it to `ori_buffer_rc_dec` at list creation — identify which of these emits the redundant element cleanup
- [ ] Trace how `ori_arc/src/lower/control_flow/for_loops/for_iterator.rs` lowers `for w in words` — does it emit `RcDec` on `w` (the loop variable) after each iteration, and is that the same element that `elem_dec_fn` will also decrement?
- [ ] Trace `ori_arc/src/aims/emit_rc/` to determine if the AIMS pipeline adds element-level RcDec that conflicts with the runtime elem_dec_fn

---

## 01.2 Fix Element Ownership Contract

**File(s):** `compiler/ori_rt/src/iterator/state.rs`, `compiler/ori_rt/src/rc/list_rc.rs`, `compiler/ori_llvm/src/codegen/arc_emitter/`

**Design decision — 2 options:**

**(a) Iterator borrows elements, collection owns** (recommended):
The iterator borrows element references without incrementing their RC. The collection retains full ownership. When the iterator is dropped, it does NOT free elements — only the collection destructor does. This matches Swift's model and is simpler.

**Why this is best:** Fewer RC operations (no per-element inc/dec during iteration), simpler ownership model, matches the borrow elision the AIMS pipeline already does for function parameters.

**Trade-off:** The collection must outlive the iterator. This is already enforced by Ori's value semantics — the `for x in list` desugaring keeps the list alive for the loop's duration.

**(b) Iterator takes ownership, collection forgets elements** (Rust IntoIter model):
The iterator takes ownership of elements. The collection's length is set to 0 so its destructor skips element cleanup.

**Downside:** Requires mutating the collection during iterator creation, which conflicts with Ori's immutable-by-default semantics and COW.

**Recommended path:** Option (a) — iterator borrows, collection owns.

- [ ] Modify `IterState::List` Drop impl to NOT call element-level RC decrement — either remove `elem_dec_fn` from the iterator path, or change how the list creates the iterator to not pass `elem_dec_fn`
- [ ] Verify `ori_buffer_rc_dec` / `ori_buffer_drop_unique` correctly handles element cleanup when no iterator has consumed elements
- [ ] Verify that when an iterator is partially consumed (e.g., `break` in a `for` loop), the collection still cleans up ALL elements
- [ ] Handle the edge case: iterator outliving collection (should not happen with Ori's value semantics, but add a debug assertion)
- [ ] Handle the `for w in words yield w` case — when yield passes the element OUT of the loop body, the yielded element's RC must be incremented (it escapes the iterator's borrow scope). Verify ARC pipeline emits RcInc on yielded elements for `[T]` where T has Drop
- [ ] Handle the `for w in words do list.push(value: w)` case — mutation consuming the element into another collection. Same concern as yield: must RcInc the element if the iterator only borrows it
- [ ] Update `ori_arc/src/aims/emit_rc/` if the AIMS pipeline currently emits element-level RcDec on loop variables — the fix must be consistent between the ARC IR level (ori_arc) and the runtime level (ori_rt)
- [ ] Verify the fix works with COW: when a list is shared (RC > 1) and one reference iterates while another holds the list, element cleanup must be correct for both paths

---

## 01.3 Fix Unwind Path Double Drop

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/emit_function.rs`, `compiler/ori_llvm/src/codegen/arc_emitter/dead_unwind.rs`

**Note:** `dead_unwind.rs` already implements `detect_dead_unwind_blocks()`, called from `emit_function.rs`. The double-drop bug may be that this function misses the J15 landing pad, or that the landing pad emits two RC decrements on the same variable within a single block (not two blocks).

J15 also found that the `@main` landing pad emits two `ori_buffer_rc_dec` calls on the same list buffer. This is a separate bug from the element double-free — this is a **buffer-level** double drop in the exception handling path.

- [ ] Trace the landing pad generation for `@main` in J15 to identify why two `ori_buffer_rc_dec` calls are emitted — use `ORI_DUMP_AFTER_LLVM=1 ori build j15.ori 2>&1 | grep -A5 'landingpad'` to see the unwind blocks
- [ ] Fix the arc_emitter to track which values have already been cleaned up on the unwind path — the likely location is `emit_function.rs` which calls `detect_dead_unwind_blocks()` at function entry
- [ ] Verify that `invoke` to `nounwind` callees does not generate unreachable landing pads (this was also flagged in J16 as LOW-2) — this is handled separately in Section 03.4
- [ ] Test with multiple `invoke` calls in the same function to ensure cleanup is correct for each
- [ ] Verify that `detect_dead_unwind_blocks()` correctly handles the J15 pattern — the issue may be two RcDec in one landing pad (not two separate landing pads)
- [ ] Determine whether the double `ori_buffer_rc_dec` is emitted by `ori_arc/src/aims/emit_rc/` (ARC IR level) or by `ori_llvm/src/codegen/arc_emitter/` (LLVM codegen level) — the fix location depends on where the duplication originates

---

## 01.4 Generalize to All [T] Where T Has Drop

The fix must work for ALL collection element types that have Drop semantics, not just `str`. The full list of types that trigger element-level RC:

| Element Type | RC Strategy | Element Drop |
|-------------|-------------|--------------|
| `str` | FatPointer (SSO-aware) | `ori_rc_dec` via SSO guard in codegen (no dedicated `ori_str_rc_dec`) |
| `[T]` | HeapPointer | `ori_buffer_rc_dec` (recursive) |
| `{K: V}` | HeapPointer | Map-specific drop |
| `Set<T>` | HeapPointer | Set-specific drop |
| Closures | Closure (env ptr) | `ori_rc_dec` on env |
| Structs with Drop fields | AggregateFields | Per-field traversal |
| Sum types with Drop payloads | InlineEnum | Tag-switch dispatch |

- [ ] Write an AOT test for `[str]` — the original J15 scenario
- [ ] Write an AOT test for `[[int]]` — nested list (list elements are themselves heap-allocated)
- [ ] Write an AOT test for list of closures — `[(int) -> int]` where closures capture heap values
- [ ] Write an AOT test for list of structs with string fields — `[{name: str, age: int}]`
- [ ] Write an AOT test for list of sum types with payloads — `[Option<str>]`
- [ ] Write an AOT test for partially consumed `[str]` — `for w in words do { if w == "stop" then break; }` — verifies both consumed and unconsumed elements are correctly cleaned up
- [ ] Write an AOT test for `for w in words yield w.length()` — yield consumes each element value; verify the yielded `int` and the source `str` are both correctly handled
- [ ] Write an AOT test for `[str]` passed to TWO functions — verifies that list RC increment on second call preserves elements for both iteration passes
- [ ] Write an AOT test for map iteration: `for (k, v) in map do ...` where keys/values are `str` — `IterState::Map` has the same `elem_dec_fn` pattern
- [ ] Write an AOT test for string iteration: `for c in s` where `s: str` — `IterState::Str` owns its data via `owns_data` flag
- [ ] Run all above tests under Valgrind (`diagnostics/valgrind-aot.sh`) to confirm zero memory errors
- [ ] Run all above tests with `ORI_CHECK_LEAKS=1` to confirm zero leaks

---

## 01.R Third Party Review Findings

- None.

---

## 01.N Completion Checklist

- [ ] `[str]` iteration and cleanup produces zero double-frees (Valgrind clean)
- [ ] `[[int]]` iteration and cleanup produces zero double-frees
- [ ] `[(int) -> int]` with capturing closures — zero double-frees
- [ ] `[{name: str}]` — zero double-frees
- [ ] `[Option<str>]` — zero double-frees
- [ ] Partially consumed iterators (via `break`) — zero leaks, zero double-frees
- [ ] `for w in words yield w.length()` — zero leaks, zero double-frees
- [ ] Same `[str]` passed to multiple functions — zero leaks, zero double-frees
- [ ] Map iteration (`for (k, v) in map`) with str keys/values — zero double-frees
- [ ] String iteration (`for c in s`) — zero leaks
- [ ] Unwind path does not double-drop list buffers
- [ ] `ORI_CHECK_LEAKS=1` reports no leaks on all test programs
- [ ] `./test-all.sh` green
- [ ] `./clippy-all.sh` green
- [ ] J15 re-run: eval and AOT produce identical results, score improves
- [ ] ARC IR verify (`ori_arc::verify()`) passes on all test programs — no RcDec on already-freed variables

**Exit Criteria:** `diagnostics/valgrind-aot.sh` on all test programs above reports "0 errors from 0 contexts" AND `ORI_CHECK_LEAKS=1` reports 0 leaks AND `./test-all.sh` reports 0 failures.
