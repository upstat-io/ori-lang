---
section: "01"
title: "Runtime COW Foundation"
status: not-started
goal: "Provide the runtime primitives that all COW collection operations depend on"
inspired_by:
  - "Swift stdlib/public/core/BridgeStorage.swift — isUniquelyReferenced pattern"
  - "Roc ori_rt refcount encoding — capacity vs refcount high-bit distinction"
  - "Lean 4 IR/ExpandResetReuse.lean — isShared instruction"
depends_on: []
sections:
  - id: "01.1"
    title: "Uniqueness Check API"
    status: not-started
  - id: "01.2"
    title: "Capacity Management Primitives"
    status: not-started
  - id: "01.3"
    title: "Growth Strategy"
    status: not-started
  - id: "01.4"
    title: "Empty Collection Sentinels"
    status: not-started
  - id: "01.5"
    title: "LLVM Runtime Declarations"
    status: not-started
  - id: "01.6"
    title: "Completion Checklist"
    status: not-started
---

# Section 01: Runtime COW Foundation

**Status:** Not Started
**Goal:** Every COW collection operation (§02-§05) can call `ori_rc_is_unique()` to branch between fast (in-place) and slow (copy) paths. Capacity management and growth strategy are available as primitives. Empty collections use zero-allocation sentinels. All primitives have LLVM declarations and are callable from codegen.

**Context:** Ori's current runtime (`compiler/ori_rt/src/lib.rs`) provides `ori_rc_alloc`, `ori_rc_inc`, `ori_rc_dec`, `ori_rc_free`, and `ori_rc_count`. Collection operations like `ori_list_push_new` unconditionally allocate new buffers. The RC header is an 8-byte `i64` (or `AtomicI64`) prepended to the data pointer. The infrastructure to *read* the refcount exists (`ori_rc_count`), but there's no dedicated uniqueness check optimized for the COW branch, and no capacity management helpers.

**Reference implementations:**
- **Swift** `stdlib/public/core/ContiguousArrayBuffer.swift`: `beginCOWMutation()` — single atomic read, returns bool. The entire COW protocol is: check → copy-if-shared → mutate → end.
- **Lean 4** `IR/ExpandResetReuse.lean`: `isShared` instruction — tests RC != 1, used to branch between fast (reuse) and slow (allocate) paths.
- **Roc** `crates/compiler/builtins/bitcode/src/list.zig`: `is_unique()` — checks `refcount_ptr.* == REFCOUNT_ONE` (the initial refcount value).

**Depends on:** Nothing — this is the foundation.

---

## 01.1 Uniqueness Check API

**File(s):** `compiler/ori_rt/src/lib.rs`

The uniqueness check is the single most important primitive. Every COW operation gates on it. It must be:
1. **Branch-predictable** — the "unique" path is the common case
2. **Minimal instruction count** — one load + one compare + one branch
3. **Thread-safe** — use `Relaxed` ordering (sufficient because we only need to know if *we* are the only owner; if another thread is racing, the answer is conservatively "not unique")

- [ ] Add `ori_rc_is_unique` function:
  ```rust
  /// Returns true if the refcount is exactly 1 (sole owner).
  /// Safe for COW: if unique, caller may mutate the allocation in place.
  /// Uses Relaxed ordering — sufficient because:
  /// - If truly unique (RC=1), no other thread can observe the value
  /// - If racing with another thread's inc, Relaxed may see 1 when it's
  ///   actually 2, but that's impossible: the inc must have happened
  ///   before the dec that brought us to 1, so if we see 1, we ARE unique.
  #[unsafe(no_mangle)]
  pub extern "C" fn ori_rc_is_unique(data: *const u8) -> bool {
      if data.is_null() {
          return false; // Null pointer is never unique (sentinel)
      }
      unsafe {
          let header = data.sub(RC_HEADER_SIZE) as *const RcHeader;
          #[cfg(feature = "multithreaded")]
          { (*header).strong_count.load(Ordering::Relaxed) == 1 }
          #[cfg(not(feature = "multithreaded"))]
          { (*header).strong_count == 1 }
      }
  }
  ```

- [ ] Add `ori_rc_is_unique_or_null` variant for sentinel-aware code:
  ```rust
  /// Returns true if data is null (sentinel) OR refcount is 1.
  /// Used by operations that handle sentinels separately.
  #[unsafe(no_mangle)]
  pub extern "C" fn ori_rc_is_unique_or_null(data: *const u8) -> bool {
      data.is_null() || ori_rc_is_unique(data)
  }
  ```

- [ ] Add unit tests:
  - Freshly allocated block → `is_unique` returns true
  - After `ori_rc_inc` → `is_unique` returns false
  - After `ori_rc_dec` back to 1 → `is_unique` returns true
  - Null pointer → `is_unique` returns false
  - Null pointer → `is_unique_or_null` returns true

---

## 01.2 Capacity Management Primitives

**File(s):** `compiler/ori_rt/src/lib.rs`

Collection growth requires reallocation. The runtime needs helpers to:
1. Grow a buffer in-place when possible (via `realloc`)
2. Allocate a new buffer with specific capacity
3. Copy elements between buffers

- [ ] Add `ori_rc_realloc` — attempts to grow an existing RC allocation:
  ```rust
  /// Attempts to resize an existing RC'd allocation.
  /// If the allocator can extend in place, returns the same pointer.
  /// Otherwise, allocates new memory, copies old data, frees old, returns new.
  /// The refcount is preserved (copied to new header if relocated).
  /// PRECONDITION: caller has verified ori_rc_is_unique(data) == true.
  #[unsafe(no_mangle)]
  pub extern "C" fn ori_rc_realloc(
      data: *mut u8,
      old_size: usize,
      new_size: usize,
      align: usize,
  ) -> *mut u8 {
      // ... implementation using std::alloc::realloc on the base pointer
      // Must account for RC_HEADER_SIZE offset
  }
  ```

- [ ] Add `ori_memcpy_elements` — typed element copy:
  ```rust
  /// Copies `count` elements of `elem_size` bytes from src to dst.
  /// Does NOT perform RC operations on elements — caller is responsible.
  #[unsafe(no_mangle)]
  pub extern "C" fn ori_memcpy_elements(
      dst: *mut u8,
      src: *const u8,
      count: usize,
      elem_size: usize,
  ) {
      unsafe {
          std::ptr::copy_nonoverlapping(src, dst, count * elem_size);
      }
  }
  ```

- [ ] Add `ori_memmove_elements` — overlapping element move (for insert/remove):
  ```rust
  /// Moves `count` elements of `elem_size` bytes from src to dst.
  /// Handles overlapping regions (for shifting elements during insert/remove).
  #[unsafe(no_mangle)]
  pub extern "C" fn ori_memmove_elements(
      dst: *mut u8,
      src: *const u8,
      count: usize,
      elem_size: usize,
  ) {
      unsafe {
          std::ptr::copy(src, dst, count * elem_size);
      }
  }
  ```

- [ ] Unit tests:
  - `ori_rc_realloc` with growth → data preserved, refcount preserved
  - `ori_rc_realloc` with shrink → data truncated correctly
  - `ori_memcpy_elements` → correct copy
  - `ori_memmove_elements` with overlap → no corruption

---

## 01.3 Growth Strategy

**File(s):** `compiler/ori_rt/src/lib.rs`

The growth strategy determines how much capacity to allocate when a collection runs out of space. This directly affects amortized performance.

**Design decision — growth factor:**

**(a) 2x doubling** (recommended — matches Rust Vec, Swift Array, Java ArrayList):
- Amortized O(1) append
- Wastes at most 50% capacity
- Simple, predictable, well-understood
- Memory: at most 2x the used size

**(b) 1.5x growth** (used by MSVC, Facebook folly):
- Slightly better memory utilization
- Still amortized O(1)
- More frequent reallocations

**(c) Phi (≈1.618) growth** (theoretical optimum for reuse):
- Allows freed blocks to be reused by subsequent allocations
- Complex to implement with integer arithmetic

**Recommended path:** Option (a), 2x doubling. It's the industry standard, simplest to implement, and well-understood by developers.

- [ ] Add growth factor constants and helper:
  ```rust
  /// Minimum initial capacity for collections.
  /// Chosen to avoid frequent reallocation for small collections
  /// while not wasting memory for single-element lists.
  const MIN_COLLECTION_CAPACITY: usize = 4;

  /// Growth factor numerator/denominator (2x = 2/1).
  const GROWTH_FACTOR_NUM: usize = 2;
  const GROWTH_FACTOR_DEN: usize = 1;

  /// Computes the next capacity for a collection that needs to hold
  /// at least `required` elements. Returns max(required, current * 2, MIN_CAPACITY).
  #[inline]
  fn next_capacity(current: usize, required: usize) -> usize {
      let doubled = current.saturating_mul(GROWTH_FACTOR_NUM) / GROWTH_FACTOR_DEN;
      doubled.max(required).max(MIN_COLLECTION_CAPACITY)
  }
  ```

- [ ] Add `ori_list_ensure_capacity` — grows a list's buffer if needed:
  ```rust
  /// Ensures the list has capacity for at least `required` elements.
  /// PRECONDITION: list data is uniquely owned (ori_rc_is_unique).
  /// If current capacity is sufficient, no-op.
  /// Otherwise, reallocates with next_capacity().
  #[unsafe(no_mangle)]
  pub extern "C" fn ori_list_ensure_capacity(
      list: *mut OriList,
      required: i64,
      elem_size: usize,
      elem_align: usize,
  ) {
      // ... realloc if cap < required
  }
  ```

- [ ] Unit tests:
  - `next_capacity(0, 1)` → `MIN_COLLECTION_CAPACITY` (4)
  - `next_capacity(4, 5)` → 8
  - `next_capacity(8, 9)` → 16
  - `next_capacity(8, 100)` → 100 (required > doubled)
  - Overflow: `next_capacity(usize::MAX / 2 + 1, 1)` → saturates, doesn't panic

---

## 01.4 Empty Collection Sentinels

**File(s):** `compiler/ori_rt/src/lib.rs`

Empty collections should not allocate. Instead, they point to a global sentinel — a static, never-freed allocation that behaves like an empty collection. This eliminates heap allocation for the very common case of `[]`, `""`, `{}`, `#{}`.

**Design: Null-pointer sentinel** (recommended — matches current Ori pattern):

The sentinel is simply a null data pointer:
- `OriList { len: 0, cap: 0, data: null }`
- `OriStr { len: 0, data: null }`
- `OriMap { len: 0, cap: 0, keys: null, values: null }`
- `OriSet { len: 0, cap: 0, data: null }`

The null pointer serves as the sentinel. `ori_rc_is_unique(null)` returns false, so any mutation on an empty collection triggers the slow path (which allocates a new buffer). This is correct because the empty sentinel has no buffer to mutate.

**Why null over a static allocation:** Simpler — no global state, no initialization, no thread-safety concerns for the sentinel itself. The tradeoff is that `ori_rc_inc(null)` and `ori_rc_dec(null)` must be no-ops (they already are, since the runtime null-checks data pointers).

- [ ] Verify `ori_rc_inc` and `ori_rc_dec` handle null data pointers (no-op):
  ```rust
  // These should already handle null via the null check in the function body.
  // Verify with explicit test.
  ```

- [ ] Add `ori_list_empty` constructor:
  ```rust
  /// Returns an empty list (sentinel). No allocation.
  #[unsafe(no_mangle)]
  pub extern "C" fn ori_list_empty() -> OriList {
      OriList { len: 0, cap: 0, data: std::ptr::null_mut() }
  }
  ```

- [ ] Add `ori_str_empty` constructor:
  ```rust
  #[unsafe(no_mangle)]
  pub extern "C" fn ori_str_empty() -> OriStr {
      OriStr { len: 0, data: std::ptr::null() }
  }
  ```

- [ ] Add `ori_map_empty` and `ori_set_empty` constructors (same pattern).

- [ ] Update all runtime functions that create empty collections to use sentinels.

- [ ] Unit tests:
  - Empty list sentinel → `len == 0`, `cap == 0`, `data == null`
  - `ori_rc_is_unique(null)` → false
  - `ori_rc_inc(null)` → no-op, no crash
  - `ori_rc_dec(null, drop_fn)` → no-op, drop_fn not called
  - Push to empty list → allocates new buffer (sentinel is not mutated)

---

## 01.5 LLVM Runtime Declarations

**File(s):** `compiler/ori_llvm/src/codegen/runtime_decl/mod.rs`

All new runtime functions must be declared in the LLVM codegen so they can be called from generated IR.

- [ ] Add `ori_rc_is_unique` declaration:
  ```rust
  // fn ori_rc_is_unique(data: *const u8) -> bool
  declare_runtime_fn!(ori_rc_is_unique, fn(ptr) -> i1);
  ```

- [ ] Add `ori_rc_realloc` declaration:
  ```rust
  // fn ori_rc_realloc(data: *mut u8, old_size: usize, new_size: usize, align: usize) -> *mut u8
  declare_runtime_fn!(ori_rc_realloc, fn(ptr, i64, i64, i64) -> ptr);
  ```

- [ ] Add `ori_memcpy_elements` declaration:
  ```rust
  declare_runtime_fn!(ori_memcpy_elements, fn(ptr, ptr, i64, i64) -> void);
  ```

- [ ] Add `ori_memmove_elements` declaration:
  ```rust
  declare_runtime_fn!(ori_memmove_elements, fn(ptr, ptr, i64, i64) -> void);
  ```

- [ ] Add `ori_list_ensure_capacity` declaration:
  ```rust
  declare_runtime_fn!(ori_list_ensure_capacity, fn(ptr, i64, i64, i64) -> void);
  ```

- [ ] Add sentinel constructor declarations (`ori_list_empty`, `ori_str_empty`, etc.).

- [ ] Update `runtime_decl/tests.rs` to verify all new declarations link correctly.

---

## 01.6 Completion Checklist

- [ ] `ori_rc_is_unique()` works correctly for RC=1, RC>1, and null pointers
- [ ] `ori_rc_realloc()` preserves data and refcount across reallocation
- [ ] `next_capacity()` implements 2x doubling with MIN_COLLECTION_CAPACITY=4
- [ ] `ori_list_ensure_capacity()` grows a unique list's buffer
- [ ] Empty sentinels work (null data pointer, no allocation)
- [ ] All new functions declared in LLVM runtime_decl
- [ ] All unit tests pass: `cargo test -p ori_rt`
- [ ] AOT integration test: create empty list, push one element, verify works
- [ ] `./test-all.sh` green (no regressions)
- [ ] `./clippy-all.sh` green

**Exit Criteria:** `ori_rc_is_unique()` is callable from LLVM-generated code. A test program that calls `ori_rc_is_unique()` and branches on the result compiles and runs correctly in AOT mode. All existing tests pass without modification.
