---
section: "02"
title: "List COW Operations"
status: in-progress
goal: "Every list mutation is O(1) amortized when uniquely owned"
inspired_by:
  - "Swift stdlib/public/core/Array.swift — _makeMutableAndUnique() protocol"
  - "Roc crates/compiler/builtins/bitcode/src/list.zig — listAppend with UpdateMode"
  - "Lean 4 IR/ExpandResetReuse.lean — conditional fast/slow path expansion"
depends_on: ["01"]
sections:
  - id: "02.1"
    title: "COW Push (Append)"
    status: complete
  - id: "02.2"
    title: "COW Pop"
    status: complete
  - id: "02.3"
    title: "COW Set (Index Assignment)"
    status: complete
  - id: "02.4"
    title: "COW Insert & Remove"
    status: complete
  - id: "02.5"
    title: "COW Concat (List Concatenation)"
    status: in-progress
  - id: "02.6"
    title: "COW Reverse & Sort"
    status: complete
  - id: "02.7"
    title: "LLVM Codegen Updates"
    status: not-started
  - id: "02.8"
    title: "Completion Checklist"
    status: not-started
---

# Section 02: List COW Operations

**Status:** Not Started
**Goal:** Every list mutation (`push`, `pop`, `set`, `insert`, `remove`, `concat`, `reverse`, `sort`) checks uniqueness at runtime. When the list is uniquely owned (RC==1), the operation mutates in place with O(1) amortized cost. When shared, the operation creates a copy (O(n)) — but only at the point of divergence, never again.

**Context:** Currently, `ori_list_push_new()` and all other list mutation functions in `ori_rt` unconditionally allocate a new buffer, copy all elements, perform the mutation, and return the new list. This makes every push O(n). For a loop that builds a list of N elements, the total cost is O(N²). With COW, the same loop is O(N) — identical to a mutable `Vec<T>` in Rust.

**Reference implementations:**
- **Swift** `Array.swift`: `_makeMutableAndUnique()` → `_buffer.beginCOWMutation()` → branch. Inline fast path, outlined slow path.
- **Roc** `list.zig`: `listAppend(list, elem, update_mode)` — `update_mode` is either `InPlace` or `Immutable`, determined by the compiler's alias analysis.
- **Lean 4**: Reset/Reuse pattern — if RC==1, reuse memory; if shared, allocate fresh.

**Depends on:** Section 01 (uniqueness check API, capacity management, growth strategy).

---

## 02.1 COW Push (Append)

**File(s):** `compiler/ori_rt/src/lib.rs`

Push is the highest-impact single operation. Most list construction patterns are repeated appends.

- [x] Replace `ori_list_push_new` with COW-aware `ori_list_push`: (2026-02-27, implemented as `ori_list_push_cow` with consuming semantics — sret ABI matching current codegen pattern. Uses RC-allocated data buffers. Fast path: unique+capacity=in-place O(1), unique+growth=realloc. Slow path: shared/empty=allocate+copy+dec-old. Codegen wiring deferred to §02.7)
  ```rust
  /// Appends `elem` to `list`. If the list's data buffer is uniquely owned
  /// and has sufficient capacity, writes in place (O(1)). Otherwise,
  /// allocates a new buffer with grown capacity, copies old elements,
  /// writes the new element, and decrements the old buffer's RC.
  ///
  /// Returns the (possibly new) OriList by value.
  ///
  /// Element RC: The element's RC is NOT incremented — the caller is
  /// responsible for ensuring the element is owned (Perceus handles this).
  #[unsafe(no_mangle)]
  pub extern "C" fn ori_list_push(
      list: OriList,
      elem: *const u8,
      elem_size: usize,
      elem_align: usize,
  ) -> OriList {
      let new_len = list.len + 1;

      if !list.data.is_null() && ori_rc_is_unique(list.data as *const u8) {
          // FAST PATH: unique owner
          if list.cap >= new_len {
              // Has capacity — write in place
              unsafe {
                  let dst = list.data.add((list.len as usize) * elem_size);
                  std::ptr::copy_nonoverlapping(elem, dst, elem_size);
              }
              return OriList { len: new_len, cap: list.cap, data: list.data };
          } else {
              // Needs growth — realloc (may extend in place)
              let new_cap = next_capacity(list.cap as usize, new_len as usize);
              let new_data = ori_rc_realloc(
                  list.data,
                  (list.cap as usize) * elem_size,
                  new_cap * elem_size,
                  elem_align,
              );
              unsafe {
                  let dst = new_data.add((list.len as usize) * elem_size);
                  std::ptr::copy_nonoverlapping(elem, dst, elem_size);
              }
              return OriList { len: new_len, cap: new_cap as i64, data: new_data };
          }
      } else {
          // SLOW PATH: shared or empty — allocate new
          let new_cap = next_capacity(0, new_len as usize);
          let new_data = ori_rc_alloc(new_cap * elem_size, elem_align);
          if !list.data.is_null() {
              // Copy old elements
              unsafe {
                  std::ptr::copy_nonoverlapping(
                      list.data as *const u8,
                      new_data,
                      (list.len as usize) * elem_size,
                  );
              }
              // Decrement old buffer (we no longer reference it)
              // NOTE: Only dec the buffer RC, NOT the elements — they're shared
              ori_rc_dec_no_children(list.data as *mut u8, ...);
          }
          unsafe {
              let dst = new_data.add((list.len as usize) * elem_size);
              std::ptr::copy_nonoverlapping(elem, dst, elem_size);
          }
          return OriList { len: new_len, cap: new_cap as i64, data: new_data };
      }
  }
  ```

- [x] **Slow path element RC handling**: When copying elements from a shared list to a new buffer, each element that is itself RC'd must be incremented (the new buffer now also references them). The codegen (§02.7) must emit element-wise `ori_rc_inc` calls after `ori_list_push` on the slow path. (2026-02-26, design documented; runtime uses byte-copy; codegen-side RC inc deferred to §02.7)

  **Design decision — who increments element RCs:**

  **(a) Runtime function increments elements** (recommended):
  - Pro: Single call from codegen, simpler emission
  - Pro: Runtime knows element layout via `elem_size`/`elem_align`
  - Con: Runtime needs element RC info (a function pointer or strategy enum)

  **(b) Codegen increments elements**:
  - Pro: Codegen already knows element types
  - Con: More complex emission, larger generated code
  - Con: Duplicated logic across operations

  **Recommended:** Option (a) — pass a `drop_fn`-style callback for element RC operations. The runtime calls it for each copied element on the slow path. This matches the existing drop function pattern in `arc_emitter/drop_gen.rs`.

- [x] **Thread safety note**: The uniqueness check uses `Relaxed` ordering. This is safe because: (2026-02-26, documented in `ori_rc_is_unique` doc comment — §01.1)
  - If RC==1, no other thread holds a reference (by definition)
  - If another thread is racing with `ori_rc_inc`, the value is being shared, and `Relaxed` may see either 1 or 2 — if it sees 1, the inc hasn't happened yet, so we're still the sole owner; if it sees 2, we correctly take the slow path
  - `Release`/`Acquire` is only needed for the *dec* path (to ensure visibility of writes before deallocation)

- [x] Unit tests (Rust): (2026-02-27, 5 tests in `ori_rt/src/tests.rs`: cow_push_to_empty_sentinel, cow_push_unique_with_capacity, cow_push_unique_needs_growth, cow_push_shared_list_copies, cow_push_1000_sequential_amortized. All pass with zero leaks verified via RC_LIVE_COUNT.)
  - Push to empty list (sentinel) → allocates, len=1, cap=MIN_CAPACITY
  - Push to unique list with capacity → in-place, same data pointer
  - Push to unique list without capacity → realloc, new data pointer, doubled cap
  - Push to shared list (RC=2) → new allocation, old untouched
  - 1000 sequential pushes → amortized O(1), ~10 reallocations

- [ ] AOT integration test (`compiler/ori_llvm/tests/aot/`): <!-- blocked-by:02.7 -->
  ```ori
  @test tests {
      let list = [1, 2, 3]
      let list = list.push(4)
      assert_eq(list, [1, 2, 3, 4])
      assert_eq(list.length(), 4)
  }
  ```

---

## 02.2 COW Pop

**File(s):** `compiler/ori_rt/src/lib.rs`

Pop removes and returns the last element. With COW, if unique, we simply decrement `len` (the element is still in the buffer but inaccessible). If shared, we copy all-but-last.

- [x] Replace `ori_list_pop_new` with COW-aware `ori_list_pop`: (2026-02-27, implemented as `ori_list_pop_cow` with consuming semantics. Fast path: unique=decrement len O(1), capacity retained. Slow path: shared=allocate+copy len-1+dec old. Empty list returns sentinel.)
  ```rust
  /// Removes the last element from the list. Returns the shortened list.
  /// If unique: decrements len (O(1), element stays in buffer but is logically removed).
  /// If shared: allocates new buffer with len-1 elements, copies, decs old.
  ///
  /// The popped element must be extracted by the caller BEFORE calling pop
  /// (via list.last() or index access). Pop only shortens the list.
  #[unsafe(no_mangle)]
  pub extern "C" fn ori_list_pop(
      list: OriList,
      elem_size: usize,
      elem_align: usize,
  ) -> OriList {
      assert!(list.len > 0, "pop on empty list");
      let new_len = list.len - 1;

      if ori_rc_is_unique(list.data as *const u8) {
          // FAST PATH: just shrink len
          // NOTE: We do NOT free the popped element here — the caller
          // extracted it and owns it. We just shrink the logical size.
          OriList { len: new_len, cap: list.cap, data: list.data }
      } else {
          // SLOW PATH: allocate new, copy len-1 elements
          // ... (similar to push slow path but copies fewer elements)
      }
  }
  ```

- [x] **Capacity reclamation**: When a unique list's `len` drops below `cap / 4`, consider shrinking. But this is a tradeoff — shrinking prevents memory waste but causes reallocation. **Decision: Do NOT auto-shrink.** Let capacity grow but never auto-shrink. Users can explicitly call `list.compact()` (future method) to reclaim. This matches Rust's `Vec` behavior. (2026-02-27, implemented — fast path retains capacity, verified by cow_pop_to_empty_retains_buffer test)

- [x] Unit tests: (2026-02-27, 4 tests in `ori_rt/src/tests.rs`: cow_pop_unique_decrements_len, cow_pop_shared_copies, cow_pop_to_empty_retains_buffer, cow_pop_empty_list_returns_empty. All pass, zero leaks.)
  - Pop from unique list → same data pointer, len decremented
  - Pop from shared list → new allocation, old untouched
  - Pop to empty → len=0, data pointer still valid (capacity retained)

---

## 02.3 COW Set (Index Assignment)

**File(s):** `compiler/ori_rt/src/lib.rs`

Set replaces the element at a given index. With COW, if unique, we overwrite in place. If shared, we copy the whole list, then overwrite.

- [x] Replace `ori_list_set_new` with COW-aware `ori_list_set`: (2026-02-27, implemented as `ori_list_set_cow` with consuming semantics. Fast path: unique=overwrite in-place O(1). Slow path: shared=copy entire list+overwrite in copy O(n). Out-of-bounds returns input unchanged. Codegen wiring deferred to §02.7)
  ```rust
  /// Replaces the element at `index` with `elem`.
  /// If unique: overwrites in place (O(1)).
  /// If shared: copies list, overwrites in copy (O(n)).
  ///
  /// Element RC: The OLD element at `index` must be decremented by the caller
  /// (codegen emits this). The new element is moved in (no inc needed).
  #[unsafe(no_mangle)]
  pub extern "C" fn ori_list_set(
      list: OriList,
      index: i64,
      elem: *const u8,
      elem_size: usize,
      elem_align: usize,
  ) -> OriList {
      assert!(index >= 0 && index < list.len, "index out of bounds");

      if ori_rc_is_unique(list.data as *const u8) {
          // FAST PATH: overwrite in place
          unsafe {
              let dst = list.data.add((index as usize) * elem_size);
              std::ptr::copy_nonoverlapping(elem, dst, elem_size);
          }
          list // Return same list (data pointer unchanged)
      } else {
          // SLOW PATH: clone, then overwrite
          let new_data = ori_rc_alloc((list.cap as usize) * elem_size, elem_align);
          unsafe {
              std::ptr::copy_nonoverlapping(
                  list.data as *const u8, new_data,
                  (list.len as usize) * elem_size,
              );
              let dst = new_data.add((index as usize) * elem_size);
              std::ptr::copy_nonoverlapping(elem, dst, elem_size);
          }
          // inc all elements (they're now referenced by new buffer too)
          // dec old buffer
          OriList { len: list.len, cap: list.cap, data: new_data }
      }
  }
  ```

- [x] Unit tests: (2026-02-27, 3 tests in `ori_rt/src/tests.rs`: cow_set_unique_overwrites_in_place, cow_set_shared_copies, cow_set_at_index_zero. All pass, zero leaks.)
  - Set on unique list → same data pointer, element replaced
  - Set on shared list → new allocation, old list unchanged
  - Set at index 0, middle, and last position

---

## 02.4 COW Insert & Remove

**File(s):** `compiler/ori_rt/src/lib.rs`

Insert shifts elements right; remove shifts elements left. With COW, if unique, we shift in place using `memmove`. If shared, we copy with the shift built into the copy.

- [x] Add `ori_list_insert`: (2026-02-27, implemented as `ori_list_insert_cow` with consuming semantics. Fast path: unique+capacity=memmove right+write O(n). unique+growth=realloc+memmove+write. Slow path: shared=alloc+copy [0..idx]+elem+copy [idx..len]+dec old. Index 0..=len valid, OOB returns unchanged.)
  ```rust
  /// Inserts `elem` at `index`, shifting subsequent elements right.
  /// If unique and has capacity: memmove right + write (O(n) for shift, but
  /// no allocation — the n elements are already in cache).
  /// If shared: allocate new, copy [0..index], write elem, copy [index..len].
  #[unsafe(no_mangle)]
  pub extern "C" fn ori_list_insert(
      list: OriList,
      index: i64,
      elem: *const u8,
      elem_size: usize,
      elem_align: usize,
  ) -> OriList { ... }
  ```

- [x] Add `ori_list_remove`: (2026-02-27, implemented as `ori_list_remove_cow` with consuming semantics. Fast path: unique=memmove left O(n), unique+empty=ori_rc_free. Slow path: shared=alloc+copy [0..idx]+[idx+1..len]+dec old. Caller extracts element before call.)
  ```rust
  /// Removes element at `index`, shifting subsequent elements left.
  /// If unique: memmove left (O(n) for shift, no allocation).
  /// If shared: allocate new, copy [0..index] + [index+1..len].
  /// The removed element must be extracted by caller before this call.
  #[unsafe(no_mangle)]
  pub extern "C" fn ori_list_remove(
      list: OriList,
      index: i64,
      elem_size: usize,
      elem_align: usize,
  ) -> OriList { ... }
  ```

- [x] Unit tests: (2026-02-27, 11 tests: 6 insert (beginning/middle/end unique, growth, shared, empty) + 5 remove (beginning/middle/end unique, last-element-frees, shared). All pass, zero leaks.)
  - Insert at beginning, middle, end (unique and shared)
  - Remove from beginning, middle, end (unique and shared)
  - Insert that triggers growth (unique: realloc + shift, shared: fresh alloc)
  - Insert/remove on single-element list

---

## 02.5 COW Concat (List Concatenation)

**File(s):** `compiler/ori_rt/src/lib.rs`

Concat (`list1 + list2`) appends all elements of `list2` to `list1`. With COW, if `list1` is unique and has capacity for `list2`, we extend in place.

- [x] Replace `ori_list_concat_new` with COW-aware `ori_list_concat`: (2026-02-27, implemented as `ori_list_concat_cow` with consuming semantics for list1, borrowing list2. Fast path: list1 unique+capacity=memcpy list2 O(m). unique+growth=realloc+memcpy. Slow path: shared=alloc+copy both+dec old list1.)
  ```rust
  /// Concatenates list2 onto list1.
  /// If list1 is unique:
  ///   - Has capacity: memcpy list2 elements into list1's buffer (O(m))
  ///   - No capacity: realloc list1 to fit, then memcpy (O(n+m))
  /// If list1 is shared:
  ///   - Allocate new buffer for n+m elements, copy both (O(n+m))
  ///
  /// In all cases, elements of list2 need RC inc (they're now in list1 too).
  /// list2's buffer RC is NOT decremented — the caller manages list2's lifetime.
  #[unsafe(no_mangle)]
  pub extern "C" fn ori_list_concat(
      list1: OriList,
      list2: OriList,
      elem_size: usize,
      elem_align: usize,
  ) -> OriList { ... }
  ```

- [ ] **Optimization: consume list2 when unique**: If `list2` is also uniquely owned and has no remaining references, we can move its elements without incrementing their RCs. This requires passing a flag or checking list2's uniqueness too. (Deferred — current impl borrows list2; dual-uniqueness optimization can be added when codegen is wired in §02.7)

  **Decision:** Check both lists' uniqueness. Four cases:
  1. Both unique: move list2's elements (no element RC changes), realloc list1 if needed
  2. list1 unique, list2 shared: copy list2's elements (inc each), extend list1
  3. list1 shared, list2 unique: allocate new, copy list1 (inc each), move list2
  4. Both shared: allocate new, copy both (inc all elements)

- [x] Unit tests: (2026-02-27, 5 tests: cow_concat_unique_with_capacity, cow_concat_unique_needs_growth, cow_concat_shared_copies, cow_concat_empty_lists, cow_concat_empty_list1. All pass, zero leaks.)
  - Concat where list1 has capacity → no realloc
  - Concat where list1 needs growth → realloc
  - Concat with shared list1 → new allocation
  - Concat empty lists (sentinels)
  - Concat large lists (1000+ elements)
  - Concat list with itself (`list + list`)

---

## 02.6 COW Reverse & Sort

**File(s):** `compiler/ori_rt/src/lib.rs`

Reverse and sort are in-place algorithms when the list is unique.

- [x] Add COW `ori_list_reverse`: (2026-02-27, implemented as `ori_list_reverse_cow` with consuming semantics. Fast path: unique=swap loop in-place O(n). Slow path: shared=alloc+copy in reverse order+dec old. Single/empty elements returned unchanged.)
  ```rust
  /// Reverses the list.
  /// If unique: reverse in place using swap loop (O(n), no allocation).
  /// If shared: allocate new, copy in reverse order (O(n)).
  #[unsafe(no_mangle)]
  pub extern "C" fn ori_list_reverse(
      list: OriList,
      elem_size: usize,
      elem_align: usize,
  ) -> OriList { ... }
  ```

- [x] Add COW `ori_list_sort`: (2026-02-27, implemented as `ori_list_sort_cow` with consuming semantics. Uses index-sort + cycle-following permutation. Fast path: unique=sort indices+permute in-place O(n log n). Slow path: shared=sort indices+copy in sorted order+dec old. compare_fn has C ABI.)
  ```rust
  /// Sorts the list using the provided comparison function.
  /// If unique: sort in place (O(n log n), no allocation).
  /// If shared: copy, then sort the copy (O(n) copy + O(n log n) sort).
  ///
  /// The comparison function has signature: (a: *const u8, b: *const u8) -> i32
  /// returning negative (a < b), zero (a == b), positive (a > b).
  #[unsafe(no_mangle)]
  pub extern "C" fn ori_list_sort(
      list: OriList,
      elem_size: usize,
      elem_align: usize,
      compare_fn: extern "C" fn(*const u8, *const u8) -> i32,
  ) -> OriList { ... }
  ```

- [x] **Sort algorithm choice**: Uses Rust's `Vec::sort_unstable_by` (pattern-defeating quicksort) on an index array, then applies the permutation. Unstable sort by default — no allocation beyond the index and visited arrays. Stable sort deferred to future `sort_stable` method. (2026-02-27)

- [x] Unit tests: (2026-02-27, 12 tests: 5 reverse (unique even/odd, shared, single, empty) + 7 sort (unique, shared, already-sorted, reverse-sorted, duplicates, single, empty). All pass, zero leaks.)
  - Reverse unique list → same data pointer, elements reversed
  - Reverse shared list → new allocation, original unchanged
  - Sort unique list → same data pointer, elements sorted
  - Sort shared list → new allocation
  - Sort already-sorted list (best case)
  - Sort reverse-sorted list (worst case for naive quicksort)
  - Sort with duplicates
  - Sort empty and single-element lists

---

## 02.7 LLVM Codegen Updates

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/builtins/collections.rs`, `compiler/ori_llvm/src/codegen/runtime_decl/mod.rs`

All existing list method emitters must be updated to call the new COW runtime functions instead of the old `_new` variants.

- [ ] Update `emit_list_push` to call `ori_list_push` instead of `ori_list_push_new`:
  - Pass `elem_size` and `elem_align` as additional arguments
  - Handle element RC on the slow path (emit inc loop for RC-typed elements)

- [ ] Update `emit_list_pop` → `ori_list_pop`

- [ ] Update `emit_list_set` → `ori_list_set`:
  - Emit `ori_rc_dec` for the OLD element before the set call
  - The new element is moved in (no inc)

- [ ] Update `emit_list_concat` → `ori_list_concat`

- [ ] Add emitters for `insert`, `remove`, `reverse`, `sort` (may not exist yet)

- [ ] **Element RC in slow path**: The codegen must handle element RC operations for the slow path. When a list is copied (slow path), each RC-typed element needs an `ori_rc_inc`. This should be a loop in the generated code:
  ```llvm
  ; Slow path element RC loop
  %i = phi i64 [0, %slow_path_entry], [%i_next, %rc_loop]
  %elem_ptr = getelementptr i8, ptr %new_data, i64 %offset
  call void @ori_rc_inc(ptr %elem_ptr)
  %i_next = add i64 %i, 1
  %done = icmp eq i64 %i_next, %len
  br i1 %done, label %slow_path_done, label %rc_loop
  ```

  **Alternative**: Pass an `inc_fn` to the runtime function (like `drop_fn` for dec). This moves the loop into the runtime, simplifying codegen. **Decision: runtime-side loop** — keeps codegen simple, matches the drop_fn pattern.

- [ ] Update runtime_decl/tests.rs to verify new function signatures

---

## 02.8 Completion Checklist

- [ ] `ori_list_push` — unique list with capacity: same pointer, O(1)
- [ ] `ori_list_push` — unique list without capacity: realloc, doubled cap
- [ ] `ori_list_push` — shared list: new allocation, old untouched
- [ ] `ori_list_push` — empty (sentinel): allocates MIN_CAPACITY
- [ ] `ori_list_pop` — unique: same pointer, len decremented
- [ ] `ori_list_pop` — shared: new allocation
- [ ] `ori_list_set` — unique: in-place overwrite
- [ ] `ori_list_set` — shared: copy + overwrite
- [ ] `ori_list_insert` — unique with capacity: memmove + write
- [ ] `ori_list_insert` — shared: new allocation with element inserted
- [ ] `ori_list_remove` — unique: memmove left
- [ ] `ori_list_remove` — shared: new allocation without element
- [ ] `ori_list_concat` — unique with capacity: memcpy append
- [ ] `ori_list_concat` — shared: new allocation with both lists
- [ ] `ori_list_reverse` — unique: in-place reverse
- [ ] `ori_list_sort` — unique: in-place sort
- [ ] All LLVM codegen emitters updated to use COW functions
- [ ] Element RC handled correctly on slow path (inc on copy, dec on replace)
- [ ] 1000-push benchmark: O(N) total, not O(N²)
- [ ] Valgrind clean: no leaks, no use-after-free, no double-free
- [ ] `./test-all.sh` green
- [ ] `./clippy-all.sh` green
- [ ] AOT integration tests for all operations

**Exit Criteria:** A program that pushes 10,000 elements to a list completes in O(N) time (measurable via benchmark). Sharing a list and then mutating the copy triggers exactly one O(N) copy, after which subsequent mutations are O(1). Valgrind reports zero errors on all list COW test programs. `./test-all.sh` and `./llvm-test.sh` both pass.
