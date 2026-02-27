---
section: "08"
title: "Collection Memory Recycling"
status: not-started
goal: "Dead collection buffers are recycled for new allocations, reducing malloc/free pressure"
inspired_by:
  - "Lean 4 IR/ExpandResetReuse.lean — conditional memory reuse for constructors"
  - "Koka Backend/C/ParcReuse.hs — Available map tracks reusable slots by size"
  - "Koka Backend/C/ParcReuseSpec.hs — field-level specialization (skip unchanged fields)"
depends_on: ["07"]
sections:
  - id: "08.1"
    title: "Extended Reset/Reuse for Collections"
    status: not-started
  - id: "08.2"
    title: "Same-Size Buffer Recycling"
    status: not-started
  - id: "08.3"
    title: "Drop Specialization"
    status: not-started
  - id: "08.4"
    title: "Cross-Operation Buffer Reuse"
    status: not-started
  - id: "08.5"
    title: "Completion Checklist"
    status: not-started
---

# Section 08: Collection Memory Recycling

**Status:** Not Started
**Goal:** When a collection is about to be freed (RC reaches 0) and a same-sized collection is about to be allocated, the dead collection's buffer is reused directly — no malloc/free roundtrip. This reduces allocator pressure and improves cache locality.

**Context:** The existing reset/reuse optimization in `ori_arc` works for constructors (structs, enums) but not for collection buffers. A `list.map(f)` that produces a same-length list currently allocates a fresh buffer even when the input list is about to be freed. With collection recycling, the old buffer is reused.

**Reference implementations:**
- **Lean 4** `ExpandResetReuse.lean`: `reset x` → `if isShared(x) { slow } else { reuse x's memory }`. The reuse token is threaded to the allocation site.
- **Koka** `ParcReuse.hs`: Tracks an `Available` map of freed buffers by size. New allocations check the map before calling `malloc`. `ParcReuseSpec.hs` specializes field writes (only write changed fields).

**Depends on:** Section 07 (static uniqueness analysis provides the foundation for knowing when values are consumed).

---

## 08.1 Extended Reset/Reuse for Collections

**File(s):** `compiler/ori_arc/src/reset_reuse/mod.rs`

The existing reset/reuse detects this pattern:
```
RcDec { var: x }       // x is about to be freed
Construct { dst, ... } // new value constructed with same layout
```
And converts to:
```
Reset { token: x }
Reuse { token: x, dst, ... }
```

For collections, the pattern is:
```
RcDec { var: old_list }           // old list about to be freed
CollectionAlloc { dst: new_list } // new list allocated with same element type
```

- [ ] Extend reset/reuse detection to recognize collection allocation patterns:
  ```rust
  /// Detects when a collection buffer is freed and a new buffer of compatible
  /// size is allocated shortly after. The old buffer can be reused.
  fn detect_collection_reuse(stmts: &[Stmt]) -> Vec<ReuseCandidate> {
      // For each RcDec of a collection:
      //   Look forward for a collection allocation with compatible element type/size
      //   If found and no intervening use of the freed variable:
      //     Emit Reset/Reuse pair
  }
  ```

- [ ] **Compatibility**: Two collection buffers are reuse-compatible if:
  - Same element size and alignment
  - Old buffer's capacity ≥ new collection's length (can fit)
  - No intervening reads of the old buffer's elements (they're being freed)

- [ ] **Runtime expansion**: Like existing reset/reuse, expand to:
  ```
  if ori_rc_is_unique(old_list.data) {
      // REUSE: clear elements, repurpose buffer
      drop_elements(old_list);  // dec each element
      // new_list.data = old_list.data (same allocation)
      // new_list.cap = old_list.cap
  } else {
      // SHARED: allocate new, dec old
      new_list.data = ori_rc_alloc(...);
      ori_rc_dec(old_list.data, ...);
  }
  ```

- [ ] Unit tests:
  - `list.map(f)` → old list buffer reused for result
  - `list.filter(f)` → old list buffer reused (may be partially filled)
  - Shared list → no reuse, fresh allocation

---

## 08.2 Same-Size Buffer Recycling

**File(s):** `compiler/ori_arc/src/reset_reuse/mod.rs`, `compiler/ori_rt/src/lib.rs`

For cases where the exact reuse pattern isn't detectable statically, a runtime buffer pool can recycle recently freed buffers.

**Design decision — static vs dynamic recycling:**

**(a) Static only** (recommended for now):
- Only reuse when the compiler can statically detect the pattern (§08.1)
- Pro: No runtime overhead, no pool management, deterministic
- Con: Misses dynamic reuse opportunities

**(b) Runtime buffer pool:**
- Maintain a per-size-class free list of recently freed buffers
- Pro: Catches dynamic reuse (e.g., across function boundaries)
- Con: Pool management overhead, memory retention, thread-safety
- Con: Complexity of deciding when to reclaim pooled buffers

**Recommended path:** Option (a) for now. Static detection covers the most common patterns (map, filter, fold). Dynamic pooling can be added later if profiling shows significant allocator pressure.

- [ ] Document the decision and the conditions under which dynamic pooling would be added

---

## 08.3 Drop Specialization

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/drop_gen.rs`, `compiler/ori_rt/src/lib.rs`

When a unique collection is dropped, we know it's the last reference. The drop can be specialized:

- [ ] **Unique drop path** (no RC operations on elements):
  ```rust
  /// When a unique collection is dropped, its elements are also unreferenced.
  /// Instead of calling ori_rc_dec on each element (which checks RC and may
  /// recursively free), we can call a specialized "unique drop" that:
  /// 1. Walks each element and calls THEIR drop function (if they have one)
  /// 2. Frees the buffer in one shot
  ///
  /// This avoids N atomic decrements for a list of N elements.
  pub extern "C" fn ori_list_drop_unique(
      list: OriList,
      elem_drop: Option<extern "C" fn(*mut u8)>,
      elem_size: usize,
      elem_align: usize,
  ) {
      if let Some(drop_fn) = elem_drop {
          for i in 0..list.len as usize {
              let elem_ptr = unsafe { list.data.add(i * elem_size) };
              drop_fn(elem_ptr);
          }
      }
      ori_rc_free(list.data, (list.cap as usize) * elem_size, elem_align);
  }
  ```

- [ ] **Shared drop path** (standard RC dec on each element):
  ```rust
  pub extern "C" fn ori_list_drop_shared(
      list: OriList,
      elem_dec: Option<extern "C" fn(*mut u8)>,
      elem_size: usize,
  ) {
      // Standard: dec each element, then dec the buffer
      if let Some(dec_fn) = elem_dec {
          for i in 0..list.len as usize {
              let elem_ptr = unsafe { list.data.add(i * elem_size) };
              dec_fn(elem_ptr);
          }
      }
      ori_rc_dec(list.data, ...);
  }
  ```

- [ ] **Integration with codegen**: The drop function generator in `drop_gen.rs` should emit a branch:
  ```
  if ori_rc_is_unique(list.data) {
      ori_list_drop_unique(...)  // Skip element RC ops
  } else {
      ori_list_drop_shared(...)  // Standard element dec
  }
  ```

  With static uniqueness info (§07), the branch can be eliminated.

- [ ] **Scalar elements optimization**: If the element type is scalar (int, float, bool, char), the element drop/dec is a no-op. The drop function should skip element iteration entirely:
  ```rust
  if elem_type.is_scalar() {
      // Just free the buffer, no element cleanup needed
      ori_rc_free(list.data, ...);
  }
  ```

- [ ] Unit tests:
  - Drop unique list of scalars → single free (no element iteration)
  - Drop unique list of strings → string drops called, then buffer freed
  - Drop shared list → element decs called, buffer dec called
  - Valgrind: no leaks, no double-free on any drop path

---

## 08.4 Cross-Operation Buffer Reuse

**File(s):** `compiler/ori_arc/src/reset_reuse/mod.rs`

Detect patterns where a collection operation consumes its input and produces same-size output:

- [ ] **`map` reuse**: `list.map(f)` where the mapped function produces elements of the same size. The old list's buffer can be reused for the result.

- [ ] **`filter` partial reuse**: `list.filter(f)` produces a list with `len ≤ original.len`. If the original is unique, reuse its buffer (the result just has a smaller `len`).

- [ ] **`sorted` reuse**: `list.sort(cmp)` is already in-place when unique (§02.6). This is a natural reuse.

- [ ] **`reversed` reuse**: Same as sort — in-place when unique.

- [ ] **Chain pattern**: `list.map(f).filter(g)` — the intermediate list from `map` is consumed by `filter`. If map reuses the original's buffer, and filter reuses map's buffer, the entire chain uses one allocation.

  **Implementation**: The ARC pipeline should detect these chains and thread reuse tokens through them.

- [ ] Unit tests:
  - `list.map(f)` with same-size elements → buffer reused
  - `list.filter(f)` → buffer reused, len reduced
  - `list.map(f).filter(g)` → single allocation for entire chain
  - Verify via allocation counter (not Valgrind)

---

## 08.5 Completion Checklist

- [ ] Collection buffer reuse detected for map/filter/sort/reverse patterns
- [ ] Reset/Reuse expansion handles collection buffers correctly
- [ ] Drop specialization: unique drop skips element RC ops
- [ ] Scalar element optimization: no element iteration on drop
- [ ] `list.map(f)` on unique list reuses buffer (allocation count = 0 new)
- [ ] `list.filter(f)` on unique list reuses buffer
- [ ] Chain patterns (map + filter) thread reuse through
- [ ] Valgrind clean on all recycling test programs
- [ ] No performance regression in `./test-all.sh`
- [ ] `./clippy-all.sh` green

**Exit Criteria:** A benchmark that maps a 10,000-element list shows zero additional allocations beyond the initial list creation (buffer reused). Drop of a 10,000-element list of scalars shows exactly 1 deallocation call (buffer only, no element iteration). Valgrind reports zero errors on all recycling test programs.
