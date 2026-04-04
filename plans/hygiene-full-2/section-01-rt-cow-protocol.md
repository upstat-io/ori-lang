---
section: "01"
title: "Runtime COW Protocol Centralization"
status: not-started
reviewed: true
goal: "Centralize the COW uniqueness check, propagate_elem_header, and write_collection_struct patterns into single canonical functions — eliminate 17+ inline copies"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "01.1"
    title: "Centralize COW Uniqueness Check"
    status: not-started
  - id: "01.2"
    title: "Unify propagate_elem_header / propagate_header"
    status: not-started
  - id: "01.3"
    title: "Unify write_collection_struct"
    status: not-started
  - id: "01.4"
    title: "Extract Iterator Consumer Loop Harness"
    status: not-started
  - id: "01.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "01.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 01: Runtime COW Protocol Centralization

**Status:** Not Started
**Goal:** Reduce 17+ inline copies of the COW uniqueness formula, 3 copies of propagate_elem_header, 3 copies of write_collection_struct, and 4 copies of the iterator consumer loop harness into single canonical functions. Zero behavioral change.

**Context:** The COW uniqueness check `!is_slice_cap(cap) && (cow_mode == 1 || (cow_mode != 2 && ori_rc_is_unique(data)))` is inlined at 17 call sites across 7 files. The `cow_mode` semantics (0=dynamic, 1=static unique, 2=static shared) have no canonical home. `propagate_elem_header` exists as two identical functions with different names (`propagate_elem_header` in cow.rs, `propagate_header` in cow_sort/mod.rs) plus a third inline copy in cow_structural.rs. `write_list_output`, `write_map_struct`, `write_set_struct` are identical 3-line functions.

---

## 01.1 Centralize COW Uniqueness Check

**File(s):** `compiler/ori_rt/src/cow_helpers.rs` (new), all COW files

Create a single canonical function:

```rust
/// Determine whether a COW mutation can proceed in-place.
///
/// cow_mode semantics:
/// - 0: dynamic — check runtime uniqueness via ori_rc_is_unique
/// - 1: static unique — always mutate in-place (compiler proved uniqueness)
/// - 2: static shared — always copy (compiler proved sharing)
///
/// Slices are never mutated in-place (they are views into another allocation).
#[inline(always)]
pub(crate) fn cow_can_mutate_in_place(data: *const u8, cap: i64, cow_mode: i32) -> bool {
    !is_slice_cap(cap) && (cow_mode == 1 || (cow_mode != 2 && ori_rc_is_unique(data)))
}
```

- [ ] Create `compiler/ori_rt/src/cow_helpers.rs` with `cow_can_mutate_in_place()`
- [ ] Add `mod cow_helpers;` to `compiler/ori_rt/src/lib.rs` (note: lib.rs is already 512 lines -- tracked in Section 08.4 for splitting; adding one `mod` line is fine here)
- [ ] Replace all 17 inline copies across these files:
  - `list/cow.rs` — `ori_list_push_cow`, `ori_list_pop_cow`, `ori_list_set_cow`
  - `list/cow_structural.rs` — `ori_list_insert_cow`, `ori_list_remove_cow`
  - `list/cow_sort/mod.rs` — `ori_list_concat_cow`, `ori_list_reverse_cow`
  - `list/cow_sort/sort.rs` — `ori_list_sort_cow`
  - `map/cow.rs` — `ori_map_insert_cow`, `ori_map_remove_cow`
  - `set/cow/basic.rs` — `ori_set_insert_cow`, `ori_set_remove_cow`
  - `set/cow/algebra.rs` — `ori_set_union_cow`, `ori_set_intersection_cow`, `ori_set_difference_cow`
- [ ] Verify: `grep -rn "cow_mode == 1\|cow_mode != 2" compiler/ori_rt/src/ | grep -v cow_helpers | grep -v test` returns 0 matches
- [ ] `timeout 150 cargo test -p ori_rt` passes after this sub-section

---

## 01.2 Unify propagate_elem_header / propagate_header

**File(s):** `compiler/ori_rt/src/cow_helpers.rs`, `compiler/ori_rt/src/list/cow.rs`, `compiler/ori_rt/src/list/cow_sort/mod.rs`, `compiler/ori_rt/src/list/cow_structural.rs`

- [ ] Move `propagate_elem_header` from `list/cow.rs` (line 21) into `cow_helpers.rs` as the single canonical copy
- [ ] Delete `propagate_header` from `list/cow_sort/mod.rs` (line 23, identical body)
- [ ] Replace the inline copy in `cow_structural.rs` with calls to `cow_helpers::propagate_elem_header` -- check both `ori_list_insert_cow` and `ori_list_remove_cow` for inline header propagation
- [ ] Update all import paths
- [ ] Verify: `grep -rn "fn propagate_header\|fn propagate_elem_header" compiler/ori_rt/src/` shows exactly 1 definition in `cow_helpers.rs`

---

## 01.3 Unify write_collection_struct

**File(s):** `compiler/ori_rt/src/cow_helpers.rs`, `compiler/ori_rt/src/list/mod.rs`, `compiler/ori_rt/src/map/mod.rs`, `compiler/ori_rt/src/set/mod.rs`

`write_list_output`, `write_map_struct`, `write_set_struct` are identical:

```rust
pub(crate) unsafe fn write_collection_struct(out: *mut u8, len: i64, cap: i64, data: *mut u8) {
    out.cast::<i64>().write(len);
    out.cast::<i64>().add(1).write(cap);
    out.add(16).cast::<*mut u8>().write(data);
}
```

- [ ] Add `write_collection_struct` to `cow_helpers.rs`
- [ ] Replace `write_list_output` in `list/mod.rs` with a re-export or inline call
- [ ] Replace `write_map_struct` in `map/mod.rs`
- [ ] Replace `write_set_struct` in `set/mod.rs`
- [ ] Replace inline writes in `list/cow.rs` `ori_list_push_cow` (which doesn't use `write_list_output`)
- [ ] Verify: all collection sret writes go through `write_collection_struct`

---

## 01.4 Extract Iterator Consumer Loop Harness

**File(s):** `compiler/ori_rt/src/iterator/consumers.rs`

`ori_iter_any`, `ori_iter_all`, `ori_iter_find`, `ori_iter_for_each` share identical loop harness: null check, cast to IterState, loop calling state.next(), test element, cleanup via Box::from_raw.

- [ ] Extract `consume_iter()` higher-order function:
  ```rust
  unsafe fn consume_iter<F, R>(
      iter: *mut u8,
      elem_size: i64,
      init: R,
      f: F,
  ) -> R
  where
      F: FnMut(R, &[u8]) -> ControlFlow<R, R>,
  ```
- [ ] Rewrite `ori_iter_any`, `ori_iter_all`, `ori_iter_find`, `ori_iter_for_each` using `consume_iter`
- [ ] Extract `collect_to_reverse_vec()` for shared collection phase in `ori_iter_rfold` and `ori_iter_rfind`
- [ ] Verify: `timeout 150 cargo test -p ori_rt` passes

### Cleanup (fix while touching these files)

- [ ] **[WASTE]** `compiler/ori_rt/src/iterator/consumers.rs:232` — Remove decorative unicode dash banner `// ── Backward consumers ...──────────`, replace with plain `// Backward consumers (require double-ended iterators)`
- [ ] **[WASTE]** `compiler/ori_rt/src/list/cow.rs` — `write_list_output` is still defined here even after `write_collection_struct` extraction; ensure it's fully removed, not left as dead code

---

## 01.R Third Party Review Findings

- None.

---

## 01.T Test Strategy

This section is pure structural refactoring with zero behavioral change. The test strategy focuses on:
1. **Existing test suite as regression gate:** `./test-all.sh` must pass identically before and after each sub-section.
2. **Unit tests for new canonical functions:** Each extracted function gets direct unit tests verifying it matches the old inline behavior.
3. **Structural invariant tests:** Grep-based tests that verify no inline copies remain.

- [ ] Add unit tests for `cow_can_mutate_in_place()` in `compiler/ori_rt/src/cow_helpers/tests.rs`:
  - `cow_mode=1` (static unique) returns `true` regardless of RC
  - `cow_mode=2` (static shared) returns `false` regardless of RC
  - `cow_mode=0` (dynamic) returns `true` when `ori_rc_is_unique` returns true
  - `cow_mode=0` (dynamic) returns `false` when refcount > 1
  - Slice cap always returns `false` regardless of cow_mode
- [ ] Add unit tests for `write_collection_struct()`: verify struct layout matches the (len, cap, data) triple at correct offsets (0, 8, 16)
- [ ] Add unit tests for `consume_iter()`: verify ControlFlow::Break stops iteration, ControlFlow::Continue processes all elements
- [ ] Verify `timeout 150 cargo test -p ori_rt` passes after each sub-section (01.1, 01.2, 01.3, 01.4)
- [ ] Verify `timeout 150 ./test-all.sh` passes after all sub-sections complete
- [ ] Verify `ORI_CHECK_LEAKS=1` reports zero leaks on COW-heavy test programs (e.g., `tests/spec/collections/cow/`)

---

## 01.N Completion Checklist

- [ ] All 17 inline COW uniqueness checks replaced with `cow_can_mutate_in_place()`
- [ ] Single `propagate_elem_header` function in `cow_helpers.rs`
- [ ] Single `write_collection_struct` function in `cow_helpers.rs`
- [ ] Iterator consumer loop harness extracted
- [ ] Unit tests for all new canonical functions pass
- [ ] `timeout 150 cargo test -p ori_rt` passes
- [ ] `timeout 150 ./test-all.sh` passes (zero behavioral changes)
- [ ] `./clippy-all.sh` clean
- [ ] `/tpr-review` covering Section 01
- [ ] `/impl-hygiene-review last commit`
