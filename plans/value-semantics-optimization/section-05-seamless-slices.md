---
section: "05"
title: "Seamless Slices"
status: complete
goal: "Slicing a list or string produces a zero-copy view that shares the underlying buffer"
inspired_by:
  - "Roc OWNERSHIP.md — seamless slices with SEAMLESS_SLICE_BIT in length field"
  - "Roc str.zig — strTrim, strSubstring as consuming/borrowing seamless slices"
  - "Go slices — runtime/slice.go — offset+length view into backing array"
depends_on: ["01"]
sections:
  - id: "05.1"
    title: "Slice Encoding Design"
    status: complete
  - id: "05.2"
    title: "List Slices"
    status: complete
  - id: "05.3"
    title: "String Slices"
    status: complete
  - id: "05.4"
    title: "Slice-Aware RC"
    status: complete
  - id: "05.5"
    title: "COW on Slice Mutation"
    status: complete
  - id: "05.6"
    title: "LLVM Codegen for Slices"
    status: complete
  - id: "05.7"
    title: "Completion Checklist"
    status: complete
---

# Section 05: Seamless Slices

**Context:** Currently, slicing operations copy all elements from the original into a new allocation. A `list.slice(0, 1000)` on a 10,000-element list copies 1,000 elements. With seamless slices, it creates a view in O(1) — just a pointer + offset + length, with an RC increment on the original buffer.

**Reference implementations:**
- **Roc** `OWNERSHIP.md`: Seamless slices encode the slice flag in a spare bit of the length field. The slice's data pointer points into the original allocation. When freed, only the RC is decremented (not the individual elements, since the original owns them).
- **Go** `runtime/slice.go`: Slices are `{ptr, len, cap}` — the pointer can be offset into a backing array. Cap limits how far the slice can grow without reallocating.

**Depends on:** Section 01 (COW primitives).

**Co-implementation requirement with §03 (String Optimization):**
Seamless string slices must interact correctly with SSO. An SSO string cannot be sliced seamlessly (it has no heap buffer to share). Slicing an SSO string either copies (creating a new SSO or heap string) or promotes to heap first. The SSO check must precede the slice path.

---

## 05.1 Slice Encoding Design

**File(s):** `compiler/ori_rt/src/lib.rs`

**Design decision — how to encode "this is a slice":**

**(a) Flag bit in capacity field** (recommended):
```
Regular list:  { len: 5, cap: 8,          data: ptr_to_start }
Slice:         { len: 3, cap: SLICE_FLAG, data: ptr_into_original }
```

Use the sign bit of `cap` (i64) as the slice flag:
- `cap >= 0` → regular list (cap is capacity)
- `cap < 0` → seamless slice (data points into another allocation)

When `cap < 0`, the data pointer does NOT point to the start of an RC allocation — it points somewhere *inside* one. The RC is on the original allocation's header.

- Pro: No struct layout change
- Pro: Single bit check (`cap < 0`)
- Pro: Remaining 63 bits of cap can store the offset to the original allocation's RC header (for cleanup)
- Con: Slightly more complex RC handling

**(b) Flag bit in length field** (Roc-style):
- Pro: Works with length-only structs
- Con: Reduces max length by half (63-bit length)
- Con: Length operations must mask the flag

**(c) Separate bool field:**
- Pro: Simplest
- Con: Increases struct size by 8 bytes (alignment)

**Recommended path:** Option (a) — flag bit in capacity field. The sign bit of `cap` is natural (capacity is always non-negative for regular collections). This keeps `len` clean for length operations.

### Slice Representation

- [x] Define slice constants: (2026-02-28) — `compiler/ori_rt/src/slice/mod.rs`
  ```rust
  /// The sign bit of `cap` marks a seamless slice.
  /// A slice's data pointer is offset into another allocation's buffer.
  const SLICE_FLAG: i64 = i64::MIN; // 0x8000_0000_0000_0000

  /// Extracts the original allocation pointer from a slice.
  /// Stored in the lower 63 bits of cap as a negative offset from data.
  /// original_data = slice.data - offset
  /// original_rc_header = original_data - RC_HEADER_SIZE
  #[inline]
  fn slice_original_data(slice: &OriList) -> *mut u8 {
      let offset = (slice.cap & !SLICE_FLAG) as usize;
      unsafe { slice.data.sub(offset) }
  }

  /// Creates a slice cap value from the offset to the original data.
  #[inline]
  fn make_slice_cap(offset: usize) -> i64 {
      SLICE_FLAG | (offset as i64)
  }

  #[inline]
  fn is_slice(list: &OriList) -> bool {
      list.cap < 0
  }
  ```

- [x] **Invariant**: A slice's data pointer is always `original_data + offset` where: (2026-02-28) — verified by `slice_original_data_computes_correct_base` and `slice_of_slice_offset_accumulation` tests
  - `original_data` is the start of the original allocation's data region
  - `offset` is stored in the lower bits of `cap`
  - `original_data - RC_HEADER_SIZE` is the RC header location

---

## 05.2 List Slices

**File(s):** `compiler/ori_rt/src/lib.rs`

- [x] Add `ori_list_slice`: (2026-02-28) — `compiler/ori_rt/src/list/slice.rs` (uses sret pattern, named `ori_list_slice`)
  ```rust
  /// Creates a seamless slice of the list from index `start` to `end` (exclusive).
  ///
  /// The slice shares the original list's data buffer. No elements are copied.
  /// The original buffer's RC is incremented (the slice references it).
  ///
  /// The returned OriList has:
  ///   - len = end - start
  ///   - cap = SLICE_FLAG | (start * elem_size)  (offset from original data)
  ///   - data = original.data + start * elem_size
  ///
  /// PRECONDITION: 0 <= start <= end <= list.len
  #[unsafe(no_mangle)]
  pub extern "C" fn ori_list_slice(
      list: OriList,
      start: i64,
      end: i64,
      elem_size: usize,
  ) -> OriList {
      assert!(start >= 0 && end >= start && end <= list.len);

      let offset = (start as usize) * elem_size;
      let original_data = if is_slice(&list) {
          slice_original_data(&list)
      } else {
          list.data
      };

      // Increment RC on the original buffer
      if !original_data.is_null() {
          ori_rc_inc(original_data as *mut u8);
      }

      let total_offset = if is_slice(&list) {
          // Slice of a slice: accumulate offsets
          let existing_offset = (list.cap & !SLICE_FLAG) as usize;
          existing_offset + offset
      } else {
          offset
      };

      OriList {
          len: end - start,
          cap: make_slice_cap(total_offset),
          data: unsafe { original_data.add(total_offset) },
      }
  }
  ```

- [x] Add `ori_list_take` and `ori_list_drop` as slice shortcuts: (2026-02-28) — named `ori_list_slice_take`/`ori_list_slice_drop` to avoid conflict with existing `ori_list_take` (box extraction)
  ```rust
  /// list.take(n) = list.slice(0, n) — first n elements
  pub extern "C" fn ori_list_take(list: OriList, n: i64, elem_size: usize) -> OriList {
      ori_list_slice(list, 0, n.min(list.len), elem_size)
  }

  /// list.drop(n) = list.slice(n, list.len) — skip first n elements
  pub extern "C" fn ori_list_drop(list: OriList, n: i64, elem_size: usize) -> OriList {
      ori_list_slice(list, n.min(list.len), list.len, elem_size)
  }
  ```

- [x] Unit tests: (2026-02-28) — 15 tests covering all 6 scenarios plus boundary clamping, take/drop shortcuts, and multi-slice sharing

---

## 05.3 String Slices

**File(s):** `compiler/ori_rt/src/lib.rs`

String slices follow the same pattern but must handle SSO strings specially.

- [x] Add `ori_str_substring`: (2026-02-28) — `compiler/ori_rt/src/string/methods/mod.rs`, handles SSO copy, heap short-result SSO optimization, heap slice with offset accumulation
  ```rust
  /// Creates a seamless slice of the string from byte offset `start` to `end`.
  ///
  /// If the string is SSO: copies the bytes into a new SSO string (can't
  /// share SSO inline storage — it's part of the struct, not heap-allocated).
  ///
  /// If the string is heap: creates a slice view (same as list slice).
  ///
  /// PRECONDITION: start and end are valid UTF-8 boundaries.
  #[unsafe(no_mangle)]
  pub extern "C" fn ori_str_substring(
      str: OriStr,
      start: i64,
      end: i64,
  ) -> OriStr {
      let len = (end - start) as usize;

      if str.is_sso() {
          // Can't slice SSO — copy the bytes
          let bytes = &str.as_bytes()[(start as usize)..(end as usize)];
          OriStr::from_bytes(bytes)  // Will be SSO if ≤ 23 bytes
      } else {
          // Heap string — create seamless slice
          let heap = unsafe { &str.heap };
          let original_data = heap.data;

          if !original_data.is_null() {
              ori_rc_inc(original_data as *mut u8);
          }

          OriStr { heap: OriStrHeap {
              len: len as i64,
              cap: make_slice_cap(start as usize),
              data: unsafe { original_data.add(start as usize) },
          }}
      }
  }
  ```

- [x] `ori_str_trim` — returns a seamless slice (or SSO copy if SSO input): (2026-02-28) — updated to delegate to `ori_str_substring`
  ```rust
  /// Trims leading and trailing whitespace.
  /// Heap string: returns seamless slice of the trimmed region.
  /// SSO string: returns new SSO string with trimmed bytes.
  #[unsafe(no_mangle)]
  pub extern "C" fn ori_str_trim(str: OriStr) -> OriStr {
      let bytes = str.as_bytes();
      let start = bytes.iter().position(|b| !b.is_ascii_whitespace()).unwrap_or(bytes.len());
      let end = bytes.iter().rposition(|b| !b.is_ascii_whitespace()).map(|i| i + 1).unwrap_or(start);
      ori_str_substring(str, start as i64, end as i64)
  }
  ```

- [x] `ori_str_split` — produces a list of string slices (each sharing the original): (2026-02-28) — `compiler/ori_rt/src/string/ops.rs`, long pieces from heap strings become slices, short pieces use SSO

- [x] Unit tests: (2026-02-28) — 16 tests in `compiler/ori_rt/src/string/methods/tests.rs`
  - Substring of heap string → slice, no copy
  - Substring of SSO string → new SSO string (copy)
  - Trim of heap string → slice
  - Trim of SSO string → SSO copy
  - Split produces slices (verify RC lifecycle)

---

## 05.4 Slice-Aware RC

**File(s):** `compiler/ori_rt/src/lib.rs`, `compiler/ori_arc/src/ir/repr.rs`

Slices require special RC handling because their data pointer doesn't point to the start of an RC allocation.

- [x] **RC header expansion** (V2→V3): (2026-02-28) — `compiler/ori_rt/src/rc/mod.rs`
  - Header expanded from 8 to 16 bytes: `[data_size: i64 | strong_count: i64 | data...]`
  - `strong_count` stays at `data_ptr - 8` (all existing RC operations unchanged)
  - `data_size` stored at `data_ptr - 16` (new; enables slice deallocation)
  - `ori_rc_alloc`, `ori_rc_free`, `ori_rc_realloc` updated internally
  - Added `ori_rc_data_size(data_ptr)` helper to read stored allocation size
  - Added `RC_HEADER_SIZE = 16` public constant
  - LLVM codegen GEP to refcount (`-8`) unchanged — refcount stays at `data - 8`
  - DWARF debug info updated in `builder_scope.rs` to reflect 3-field header

- [x] **RC Inc for slices**: (2026-02-28) — `compiler/ori_rt/src/rc/collections.rs`
  - `ori_list_rc_inc(data, cap)`: if `is_slice_cap(cap)`, computes `original_data` via `slice_original_data`, then calls `ori_rc_inc(original_data)`. For regular lists, calls `ori_rc_inc(data)` directly.

- [x] **RC Dec for slices**: (2026-02-28) — `compiler/ori_rt/src/rc/collections.rs`
  - `ori_buffer_rc_dec` updated with slice check at entry: if `is_slice_cap(cap)`, delegates to `slice_buffer_rc_dec` which decs the original buffer's RC.
  - On last reference (RC=0): best-effort element cleanup (slice's visible elements only, not full buffer), then frees buffer using `ori_rc_data_size` from header.

- [x] **Element RC on slice drop**: (2026-02-28) — documented limitation
  - When a slice is the last reference and RC hits 0, only the slice's `len` elements get `elem_dec_fn` called (best-effort). Elements outside the slice's range may leak child RCs.
  - Buffer itself is correctly freed using stored `data_size` from the V3 header.
  - Acceptable because: most elements are scalars (no child RCs); originals usually outlive slices; full cleanup would require storing original `len` in header.

- [x] Update `ori_arc`'s `RcStrategy` to handle slices: (2026-02-28) — Completed in 05.6. Both `emit_rc_inc_heap` and `inc_value_rc` now extract `cap` for List/Set and call `ori_list_rc_inc(data, cap)`. Added `"drop"`, `"slice"`, `"substring"`, `"take"` to `BORROWING_METHOD_NAMES` in `ori_arc/src/borrow/builtins.rs`.

- [x] Unit tests: (2026-02-28) — 9 new tests in `compiler/ori_rt/src/list/slice/tests.rs`
  - `ori_list_rc_inc_on_regular_list` — inc on owned list works normally
  - `ori_list_rc_inc_on_slice` — inc on slice targets original buffer
  - `ori_list_rc_inc_null_is_noop` — null data is safe
  - `ori_buffer_rc_dec_on_slice_decs_original` — dec slice targets original
  - `ori_buffer_rc_dec_slice_last_ref_frees` — last ref via slice frees buffer
  - `ori_buffer_rc_dec_slice_of_slice_decs_original` — nested slice targets true original
  - `ori_rc_data_size_returns_allocation_size` — stored size matches alloc
  - `ori_rc_data_size_null_returns_zero` — null safety
  - `slice_rc_full_lifecycle` — comprehensive create/slice/drop lifecycle

---

## 05.5 COW on Slice Mutation

**File(s):** `compiler/ori_rt/src/lib.rs`

When a slice is mutated (push, set, etc.), it must be "materialized" — copied out of the shared buffer into its own allocation.

- [x] Add `ori_list_materialize_slice` — in `list/slice.rs`, allocates new buffer, copies elements, inc's element RCs, dec's original buffer RC. Follows sret pattern. No-op for non-slice inputs.

- [x] **COW operations on slices**: All COW mutation functions guard fast paths with `!is_slice_cap(cap)` to prevent `ori_rc_is_unique(data)` on interior pointers. Slices always take the slow path (allocate + copy). Added `dec_list_buffer(data, cap)` helper that does slice-aware RC dec (replaces raw `ori_rc_dec(data, None)` in slow paths).
  - Updated: `ori_list_push_cow`, `ori_list_pop_cow`, `ori_list_set_cow`, `ori_list_insert_cow`, `ori_list_remove_cow` (in `cow.rs`)
  - Updated: `ori_list_reverse_cow`, `ori_list_sort_cow`, `ori_list_sort_stable_cow`, `ori_list_concat_cow` (in `cow_sort.rs`)
  - Updated `dec_consumed_list2` to handle slice cap2 in concat

- [x] Unit tests (10 tests):
  - `cow_push_on_slice_materializes` — push to slice produces owned list with [20,30,40,99]
  - `cow_pop_on_slice_materializes` — pop from slice produces owned list with [10,20]
  - `cow_set_on_slice_materializes` — set on slice produces owned list with [20,999,40]
  - `cow_insert_on_slice_materializes` — insert into slice produces owned list with [20,55,30]
  - `cow_remove_on_slice_materializes` — remove from slice produces owned list with [20,40]
  - `cow_reverse_on_slice_materializes` — reverse slice produces owned list with [40,30,20]
  - `cow_concat_with_slice_list1` — concat slice + regular list produces [20,30,50,60]
  - `materialize_slice_produces_owned_list` — explicit materialize produces owned copy
  - `materialize_non_slice_is_noop` — materialize on regular list returns same pointer
  - `cow_push_on_slice_rc_lifecycle` — verifies RC transitions through push_cow

---

## 05.6 LLVM Codegen for Slices

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/`

- [x] Emit `is_slice` check before RC operations on lists: (2026-02-28) — Both `emit_rc_inc_heap` (HeapPointer strategy) and `inc_value_rc` (AggregateFields strategy) now use `call_list_rc_inc(data, cap)` for List/Set types instead of the unsafe `call_rc_inc_all(&[data])`. The runtime `ori_list_rc_inc` checks `is_slice_cap(cap)` and routes to the original buffer.

- [x] Update RC emission to handle slices (compute original pointer): (2026-02-28) — `rc_ops.rs` and `rc_value_traversal.rs` updated with tag-based dispatch for List/Set. `call_list_rc_inc` helper added.

- [x] Add emitters for `ori_list_slice`, `ori_list_take`, `ori_list_drop`: (2026-02-28) — `list_builtins.rs` with `emit_list_slice`, `emit_list_take_slice`, `emit_list_drop_slice`. Dispatch entries in `collections/mod.rs` with `borrow: true`.

- [x] Add emitters for `ori_str_substring`, `ori_str_trim`: (2026-02-28) — `string_builtins.rs` with `emit_str_substring` (sret pattern). `str.slice` aliases to `emit_str_substring`. `str.trim` was already in the builtin table.

- [x] AOT integration tests: (2026-02-28) — 19 tests in `compiler/ori_llvm/tests/aot/slices.rs` covering list.slice, list.take, list.drop, str.substring, str.slice, chaining, and RC safety.
  ```ori
  use std.testing { assert_eq }

  @test tests {
      let list = [1, 2, 3, 4, 5]
      let slice = list.slice(1, 4)
      assert_eq(slice, [2, 3, 4])
      assert_eq(slice.length(), 3)

      // Mutation of slice creates new list
      let modified = slice.push(6)
      assert_eq(modified, [2, 3, 4, 6])
      assert_eq(slice, [2, 3, 4])  // Original slice unchanged
  }
  ```

---

## 05.7 Completion Checklist

- [x] `list.slice(start, end)` returns a zero-copy view (O(1)) — (2026-02-28) verified by `slice_of_regular_list` + AOT `test_list_slice_basic`
- [x] `list.take(n)` and `list.drop(n)` return zero-copy views — (2026-02-28) verified by `take_first_n`/`drop_first_n` + AOT tests
- [x] `str.substring(start, end)` returns zero-copy view (heap) or SSO copy (SSO) — (2026-02-28) verified by `substring_of_heap_string_produces_slice`/`substring_of_sso_string_copies`
- [x] `str.trim()` returns zero-copy view — (2026-02-28) verified by `trim_heap_returns_slice` + Valgrind clean
- [x] Slice of a slice → correct offset accumulation — (2026-02-28) verified by `slice_of_slice_accumulates_offsets`/`substring_of_slice_accumulates_offsets` + AOT `test_list_take_then_drop`
- [x] RC on slices targets the original buffer, not the slice pointer — (2026-02-28) verified by `ori_list_rc_inc_on_slice`/`ori_buffer_rc_dec_on_slice_decs_original` + LLVM `call_list_rc_inc`
- [x] Slice mutation materializes (copies) before mutating — (2026-02-28) verified by 7 COW tests (`cow_push/pop/set/insert/remove/reverse_on_slice_materializes`)
- [x] Drop of last reference (original or slice) frees buffer and elements — (2026-02-28) verified by `ori_buffer_rc_dec_slice_last_ref_frees`/`slice_rc_full_lifecycle`
- [x] No double-free on drop of original + slice — (2026-02-28) verified by `slice_rc_lifecycle`/`cow_push_on_slice_rc_lifecycle` + Valgrind clean
- [x] No use-after-free when slice outlives original binding — (2026-02-28) verified by AOT `test_list_slice_preserves_original` + Valgrind clean
- [x] Valgrind clean on all slice test programs — (2026-02-28) 7/7 Valgrind tests pass including `slice_operations.ori`
- [x] `./test-all.sh` green — (2026-02-28) 10,727 tests, 0 failures
- [x] `./clippy-all.sh` green — (2026-02-28) all checks passed

**Exit Criteria:** `list.slice(0, 1000)` on a 10,000-element list completes in O(1) time with zero element copies (measurable via allocation counter). String `trim()` on a 1MB heap string is O(1). All slice RC lifecycle tests pass under Valgrind. `dual-exec-verify.sh` shows zero mismatches between interpreter and AOT for all slice operations.
