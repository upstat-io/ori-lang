---
section: "05"
title: "Seamless Slices"
status: not-started
goal: "Slicing a list or string produces a zero-copy view that shares the underlying buffer"
inspired_by:
  - "Roc OWNERSHIP.md — seamless slices with SEAMLESS_SLICE_BIT in length field"
  - "Roc str.zig — strTrim, strSubstring as consuming/borrowing seamless slices"
  - "Go slices — runtime/slice.go — offset+length view into backing array"
depends_on: ["01"]
sections:
  - id: "05.1"
    title: "Slice Encoding Design"
    status: not-started
  - id: "05.2"
    title: "List Slices"
    status: not-started
  - id: "05.3"
    title: "String Slices"
    status: not-started
  - id: "05.4"
    title: "Slice-Aware RC"
    status: not-started
  - id: "05.5"
    title: "COW on Slice Mutation"
    status: not-started
  - id: "05.6"
    title: "LLVM Codegen for Slices"
    status: not-started
  - id: "05.7"
    title: "Completion Checklist"
    status: not-started
---

# Section 05: Seamless Slices

**Status:** Not Started
**Goal:** `list.slice(start, end)`, `str.substring(start, end)`, `str.trim()`, and other view-producing operations return zero-copy views that share the underlying buffer. No allocation, no element copying. COW kicks in only if the slice is mutated.

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

- [ ] Define slice constants:
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

- [ ] **Invariant**: A slice's data pointer is always `original_data + offset` where:
  - `original_data` is the start of the original allocation's data region
  - `offset` is stored in the lower bits of `cap`
  - `original_data - RC_HEADER_SIZE` is the RC header location

---

## 05.2 List Slices

**File(s):** `compiler/ori_rt/src/lib.rs`

- [ ] Add `ori_list_slice`:
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

- [ ] Add `ori_list_take` and `ori_list_drop` as slice shortcuts:
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

- [ ] Unit tests:
  - Slice of regular list → slice flag set, data offset correct
  - Slice of a slice → offsets accumulate correctly
  - Slice with start=0, end=len → full view (same data, still a slice)
  - Slice with start=end → empty slice
  - Original list modified after slice → slice sees original data (shared)
  - RC lifecycle: slice creation incs, slice drop decs, last drop frees

---

## 05.3 String Slices

**File(s):** `compiler/ori_rt/src/lib.rs`

String slices follow the same pattern but must handle SSO strings specially.

- [ ] Add `ori_str_substring`:
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

- [ ] `ori_str_trim` — returns a seamless slice (or SSO copy if SSO input):
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

- [ ] `ori_str_split` — produces a list of string slices (each sharing the original)

- [ ] Unit tests:
  - Substring of heap string → slice, no copy
  - Substring of SSO string → new SSO string (copy)
  - Trim of heap string → slice
  - Trim of SSO string → SSO copy
  - Split produces slices (verify RC lifecycle)

---

## 05.4 Slice-Aware RC

**File(s):** `compiler/ori_rt/src/lib.rs`, `compiler/ori_arc/src/ir/repr.rs`

Slices require special RC handling because their data pointer doesn't point to the start of an RC allocation.

- [ ] **RC Inc for slices**: `ori_rc_inc` must increment the *original* allocation's RC, not the slice's data pointer:
  ```rust
  /// Increments RC on the backing allocation of a list (or slice).
  /// For regular lists: ori_rc_inc(list.data)
  /// For slices: ori_rc_inc(original_data) where original_data is computed from offset
  #[unsafe(no_mangle)]
  pub extern "C" fn ori_list_rc_inc(list: *const OriList) {
      let list = unsafe { &*list };
      if list.data.is_null() { return; }
      let rc_target = if is_slice(list) {
          slice_original_data(list)
      } else {
          list.data
      };
      ori_rc_inc(rc_target as *mut u8);
  }
  ```

- [ ] **RC Dec for slices**: Same logic — dec the original, not the slice data pointer. On drop, a slice only decrements the original buffer's RC. It does NOT dec individual elements (the original buffer owns them).

  ```rust
  #[unsafe(no_mangle)]
  pub extern "C" fn ori_list_rc_dec(list: *const OriList, drop_fn: ...) {
      let list = unsafe { &*list };
      if list.data.is_null() { return; }
      let rc_target = if is_slice(list) {
          slice_original_data(list)
      } else {
          list.data
      };
      ori_rc_dec(rc_target as *mut u8, drop_fn);
  }
  ```

- [ ] **Element RC on slice drop**: When a slice is dropped and it's the LAST reference to the original buffer, the drop function is called. The drop function walks ALL elements in the original buffer (not just the slice's range) because the original buffer owned all elements. This is correct because:
  - Slices don't own elements — the original buffer does
  - When the original buffer's RC reaches 0, ALL elements are freed
  - A slice's RC inc/dec only affects the buffer's RC, not elements

- [ ] Update `ori_arc`'s `RcStrategy` to handle slices:
  ```rust
  enum RcStrategy {
      HeapPointer,
      FatPointer,
      Closure,
      AggregateFields(Vec<FieldRcInfo>),
      InlineEnum(Vec<VariantRcInfo>),
      Slice,  // NEW: check is_slice, compute original, inc/dec original
  }
  ```

- [ ] Unit tests:
  - Create list, slice, drop slice → original RC decremented
  - Create list, slice, drop original → slice still valid (RC > 0)
  - Drop last reference (either original or slice) → buffer freed, elements freed
  - Slice of a slice → RC on original (not intermediate)
  - RC count tracking through slice lifecycle

---

## 05.5 COW on Slice Mutation

**File(s):** `compiler/ori_rt/src/lib.rs`

When a slice is mutated (push, set, etc.), it must be "materialized" — copied out of the shared buffer into its own allocation.

- [ ] Add `ori_list_materialize_slice`:
  ```rust
  /// Materializes a slice into a standalone list.
  /// Allocates a new buffer, copies the slice's elements, returns a regular list.
  /// Element RCs are incremented (new buffer now references them too).
  /// The original buffer's RC is decremented (slice no longer references it).
  #[unsafe(no_mangle)]
  pub extern "C" fn ori_list_materialize_slice(
      slice: OriList,
      elem_size: usize,
      elem_align: usize,
  ) -> OriList {
      assert!(is_slice(&slice));
      let new_cap = next_capacity(0, slice.len as usize);
      let new_data = ori_rc_alloc(new_cap * elem_size, elem_align);
      unsafe {
          std::ptr::copy_nonoverlapping(
              slice.data as *const u8,
              new_data,
              (slice.len as usize) * elem_size,
          );
      }
      // Inc elements (they're now in new buffer too)
      // Dec original buffer RC
      OriList { len: slice.len, cap: new_cap as i64, data: new_data }
  }
  ```

- [ ] **COW operations on slices**: All COW mutation functions (push, pop, set, etc.) must check `is_slice()` in addition to `ori_rc_is_unique()`:
  ```rust
  // In ori_list_push:
  if is_slice(&list) {
      // Materialize slice first, then push to the materialized copy
      let materialized = ori_list_materialize_slice(list, elem_size, elem_align);
      return ori_list_push(materialized, elem, elem_size, elem_align);
  }
  ```

  **Optimization**: The materialize + push can be fused into a single allocation:
  ```rust
  if is_slice(&list) {
      // Allocate for len+1, copy slice elements, append new element
      let new_len = list.len + 1;
      let new_cap = next_capacity(0, new_len as usize);
      let new_data = ori_rc_alloc(new_cap * elem_size, elem_align);
      // Copy slice elements + new element in one pass
      // ...
  }
  ```

- [ ] Unit tests:
  - Push to slice → materializes + pushes
  - Set on slice → materializes + sets
  - Materialize preserves element values
  - Original buffer unaffected by slice mutation
  - RC lifecycle correct through materialize

---

## 05.6 LLVM Codegen for Slices

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/`

- [ ] Emit `is_slice` check before RC operations on lists:
  ```llvm
  %cap = extractvalue %OriList %list, 1
  %is_slice = icmp slt i64 %cap, 0
  br i1 %is_slice, label %slice_rc, label %regular_rc
  ```

- [ ] Update RC emission to handle slices (compute original pointer)

- [ ] Add emitters for `ori_list_slice`, `ori_list_take`, `ori_list_drop`

- [ ] Add emitters for `ori_str_substring`, `ori_str_trim`

- [ ] AOT integration tests:
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

- [ ] `list.slice(start, end)` returns a zero-copy view (O(1))
- [ ] `list.take(n)` and `list.drop(n)` return zero-copy views
- [ ] `str.substring(start, end)` returns zero-copy view (heap) or SSO copy (SSO)
- [ ] `str.trim()` returns zero-copy view
- [ ] Slice of a slice → correct offset accumulation
- [ ] RC on slices targets the original buffer, not the slice pointer
- [ ] Slice mutation materializes (copies) before mutating
- [ ] Drop of last reference (original or slice) frees buffer and elements
- [ ] No double-free on drop of original + slice
- [ ] No use-after-free when slice outlives original binding
- [ ] Valgrind clean on all slice test programs
- [ ] `./test-all.sh` green
- [ ] `./clippy-all.sh` green

**Exit Criteria:** `list.slice(0, 1000)` on a 10,000-element list completes in O(1) time with zero element copies (measurable via allocation counter). String `trim()` on a 1MB heap string is O(1). All slice RC lifecycle tests pass under Valgrind. `dual-exec-verify.sh` shows zero mismatches between interpreter and AOT for all slice operations.
