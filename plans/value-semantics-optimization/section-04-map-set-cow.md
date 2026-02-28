---
section: "04"
title: "Map & Set COW Operations"
status: in-progress
goal: "Every map/set mutation is O(1) amortized when uniquely owned"
inspired_by:
  - "Swift stdlib/public/core/Dictionary.swift — COW with isUniquelyReferenced"
  - "Roc crates/compiler/builtins/bitcode/src/dict.zig — UpdateMode for dict operations"
depends_on: ["01"]
sections:
  - id: "04.1"
    title: "Map Layout & COW Strategy"
    status: complete
  - id: "04.2"
    title: "COW Map Insert"
    status: not-started
  - id: "04.3"
    title: "COW Map Remove & Update"
    status: not-started
  - id: "04.4"
    title: "COW Set Operations"
    status: not-started
  - id: "04.5"
    title: "Set Algebra (Union, Intersection, Difference)"
    status: not-started
  - id: "04.6"
    title: "LLVM Codegen Updates"
    status: not-started
  - id: "04.7"
    title: "Completion Checklist"
    status: not-started
---

# Section 04: Map & Set COW Operations

**Status:** Not Started
**Goal:** Map and set mutations (`insert`, `remove`, `update`, `union`, `intersection`, `difference`) check uniqueness and mutate in place when uniquely owned. Shared maps/sets copy on first mutation, then all subsequent mutations on the copy are in-place.

**Context:** Currently, `OriMap` uses two parallel arrays (keys and values) with O(n) linear scan lookup, and `OriSet` uses a single sorted array. All mutations allocate unconditionally. Map/set COW follows the same pattern as list COW (§02) but with additional complexity for key lookup and set algebra.

**Reference implementations:**
- **Swift** `Dictionary.swift`: COW via `isUniquelyReferenced`. Uses a hash table internally, but the COW mechanics are independent of the hash table.
- **Roc** `dict.zig`: COW with `UpdateMode` parameter. Uses Robin Hood hashing.

**Depends on:** Section 01 (COW primitives).

**Note on hash tables:** The current parallel-array layout has O(n) lookup. A hash table upgrade is an orthogonal concern (data structure, not mutation strategy). This section implements COW for the *current* layout. When hash tables are added (separate plan), the COW mechanics will transfer directly.

---

## 04.1 Map Layout & COW Strategy

**File(s):** `compiler/ori_rt/src/lib.rs`

Current `OriMap` layout:
```rust
pub struct OriMap {
    pub len: i64,
    pub cap: i64,
    pub keys: *mut u8,    // RC'd buffer of key elements
    pub values: *mut u8,  // RC'd buffer of value elements
}
```

Both `keys` and `values` are separate RC'd allocations. For COW, we need both to be unique.

**Design decision — single vs dual allocation:**

**(a) Keep dual allocation** (current):
- Pro: No layout change
- Con: Two RC checks needed (both keys AND values must be unique)
- Con: Two reallocs on growth

**(b) Single allocation with keys and values interleaved** (recommended):
```
┌──────────┬───────────────────────────────────────┐
│ RC header│ key0 val0 │ key1 val1 │ key2 val2 │...│
└──────────┴───────────────────────────────────────┘
```
- Pro: Single RC check (one allocation = one uniqueness check)
- Pro: Better cache locality (key and value adjacent)
- Pro: Single realloc on growth
- Con: Layout change, more complex element access (stride = key_size + val_size)

**(c) Single allocation with keys then values** (compromise):
```
┌──────────┬────────────────────┬────────────────────┐
│ RC header│ key0 key1 key2 ... │ val0 val1 val2 ... │
└──────────┴────────────────────┴────────────────────┘
```
- Pro: Single RC check, single realloc
- Pro: Keys are contiguous (better for search)
- Con: Values offset depends on key count and size

**Recommended path:** Option (c) — single allocation with keys then values. This gives a single uniqueness check and keeps keys contiguous for search. The value offset is `cap * key_size`.

- [x] Redesign `OriMap` layout:
  ```rust
  pub struct OriMap {
      pub len: i64,
      pub cap: i64,
      pub data: *mut u8,  // Single RC'd buffer: [keys...][values...]
      // keys start at data + 0
      // values start at data + cap * key_size
  }
  ```

- [x] Add key/value access helpers:
  ```rust
  impl OriMap {
      #[inline]
      fn key_ptr(&self, index: usize, key_size: usize) -> *const u8 {
          unsafe { self.data.add(index * key_size) }
      }

      #[inline]
      fn value_ptr(&self, index: usize, key_size: usize, val_size: usize) -> *const u8 {
          unsafe { self.data.add((self.cap as usize) * key_size + index * val_size) }
      }
  }
  ```

---

## 04.2 COW Map Insert

**File(s):** `compiler/ori_rt/src/lib.rs`

- [ ] Replace `ori_map_insert` with COW-aware version:
  ```rust
  /// Inserts a key-value pair into the map.
  ///
  /// If key already exists:
  ///   - Unique: overwrite value in place (O(n) search + O(1) write)
  ///   - Shared: copy all, overwrite in copy
  ///
  /// If key is new:
  ///   - Unique with capacity: append key+value (O(n) search + O(1) write)
  ///   - Unique without capacity: realloc + append
  ///   - Shared: allocate new, copy all, append
  ///
  /// Returns: OriMap (possibly new allocation)
  #[unsafe(no_mangle)]
  pub extern "C" fn ori_map_insert(
      map: OriMap,
      key: *const u8,
      value: *const u8,
      key_size: usize,
      val_size: usize,
      key_eq: extern "C" fn(*const u8, *const u8) -> bool,
  ) -> OriMap { ... }
  ```

- [ ] **Key equality function**: Map insert needs to compare keys. The codegen passes a function pointer for key equality (type-specific). This matches the existing pattern for sort comparators.

- [ ] Unit tests:
  - Insert into empty map → allocates
  - Insert new key into unique map with capacity → in-place
  - Insert existing key into unique map → overwrite in place
  - Insert into shared map → copy, original unchanged
  - Insert 1000 entries sequentially → amortized O(1)

---

## 04.3 COW Map Remove & Update

**File(s):** `compiler/ori_rt/src/lib.rs`

- [ ] Add COW `ori_map_remove`:
  ```rust
  /// Removes the entry with the given key.
  /// If unique: shift remaining entries left (O(n) for shift, no allocation).
  /// If shared: allocate new, copy all except removed entry.
  /// Returns: OriMap with the entry removed.
  #[unsafe(no_mangle)]
  pub extern "C" fn ori_map_remove(
      map: OriMap,
      key: *const u8,
      key_size: usize,
      val_size: usize,
      key_eq: extern "C" fn(*const u8, *const u8) -> bool,
  ) -> OriMap { ... }
  ```

- [ ] Add COW `ori_map_update`:
  ```rust
  /// Updates the value for an existing key using a transformation function.
  /// Equivalent to: map.insert(key, transform(map.get(key)))
  /// But avoids the double lookup and potential double COW copy.
  #[unsafe(no_mangle)]
  pub extern "C" fn ori_map_update(
      map: OriMap,
      key: *const u8,
      key_size: usize,
      val_size: usize,
      key_eq: extern "C" fn(*const u8, *const u8) -> bool,
      transform: extern "C" fn(*const u8) -> *const u8,
  ) -> OriMap { ... }
  ```

- [ ] Unit tests for remove and update with unique and shared maps

---

## 04.4 COW Set Operations

**File(s):** `compiler/ori_rt/src/lib.rs`

Sets follow the same COW pattern as maps but with simpler element handling (keys only, no values).

- [ ] Replace `ori_set_insert` with COW version:
  ```rust
  /// Inserts an element into the set.
  /// If element exists: no-op (return same set).
  /// If unique with capacity: append element (O(n) search + O(1) write).
  /// If shared: copy all, append.
  #[unsafe(no_mangle)]
  pub extern "C" fn ori_set_insert(
      set: OriSet,
      elem: *const u8,
      elem_size: usize,
      elem_eq: extern "C" fn(*const u8, *const u8) -> bool,
  ) -> OriSet { ... }
  ```

- [ ] Replace `ori_set_remove` with COW version:
  ```rust
  /// Removes an element from the set.
  /// If unique: shift left (O(n)).
  /// If shared: copy all except removed.
  #[unsafe(no_mangle)]
  pub extern "C" fn ori_set_remove(
      set: OriSet,
      elem: *const u8,
      elem_size: usize,
      elem_eq: extern "C" fn(*const u8, *const u8) -> bool,
  ) -> OriSet { ... }
  ```

- [ ] Unit tests for insert and remove with unique and shared sets

---

## 04.5 Set Algebra (Union, Intersection, Difference)

**File(s):** `compiler/ori_rt/src/lib.rs`

Set algebra operations can benefit from COW when one operand is unique.

- [ ] Replace `ori_set_union` with COW version:
  ```rust
  /// Computes set1 ∪ set2.
  /// If set1 is unique: extend set1 with elements from set2 not in set1.
  /// If shared: allocate new set with combined elements.
  ///
  /// Time: O(n*m) with linear scan. Acceptable for current layout;
  /// hash table upgrade (separate plan) would make this O(n+m).
  #[unsafe(no_mangle)]
  pub extern "C" fn ori_set_union(
      set1: OriSet,
      set2: OriSet,
      elem_size: usize,
      elem_eq: extern "C" fn(*const u8, *const u8) -> bool,
  ) -> OriSet { ... }
  ```

- [ ] Replace `ori_set_intersection` with COW version:
  ```rust
  /// Computes set1 ∩ set2.
  /// If set1 is unique: remove elements not in set2 (shrink in place).
  /// If shared: allocate new set with shared elements.
  #[unsafe(no_mangle)]
  pub extern "C" fn ori_set_intersection(...) -> OriSet { ... }
  ```

- [ ] Replace `ori_set_difference` with COW version:
  ```rust
  /// Computes set1 \ set2.
  /// If set1 is unique: remove elements in set2 (shrink in place).
  /// If shared: allocate new set with remaining elements.
  #[unsafe(no_mangle)]
  pub extern "C" fn ori_set_difference(...) -> OriSet { ... }
  ```

- [ ] Unit tests:
  - Union of unique sets → in-place extension
  - Union of shared set → new allocation
  - Intersection with unique → in-place shrink
  - Difference with unique → in-place shrink
  - Empty set operations (identity laws: A∪∅=A, A∩∅=∅, A\∅=A)
  - Self operations (A∪A=A, A∩A=A, A\A=∅)

---

## 04.6 LLVM Codegen Updates

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/builtins/collections.rs`

- [ ] Update all map method emitters to call new COW runtime functions
- [ ] Update all set method emitters to call new COW runtime functions
- [ ] Pass `key_eq` / `elem_eq` function pointers from codegen:
  - For primitive types (int, float, bool, char): emit inline comparison
  - For string keys: pass `ori_str_eq` as the comparator
  - For complex types: generate comparison function from type info
- [ ] Update `OriMap` LLVM type if layout changes (single allocation)
- [ ] AOT integration tests for map and set operations

---

## 04.7 Completion Checklist

- [ ] Map insert on unique map with capacity → in-place, same pointer
- [ ] Map insert on shared map → new allocation, original unchanged
- [ ] Map remove on unique map → in-place shift
- [ ] Map remove on shared map → new allocation
- [ ] Set insert/remove follow same COW pattern
- [ ] Union of unique set → in-place extension
- [ ] Intersection/difference of unique set → in-place shrink
- [ ] All set algebra identity laws pass (A∪∅=A, A∩∅=∅, etc.)
- [ ] Key equality handled correctly for all key types
- [ ] Element RC correct on COW paths (inc on copy, dec on remove)
- [ ] 1000-insert benchmark: O(N) total for unique map
- [ ] Valgrind clean on all map/set COW tests
- [ ] `./test-all.sh` green
- [ ] `./clippy-all.sh` green

**Exit Criteria:** A program that inserts 10,000 entries into a map completes in O(N) time when the map is uniquely owned. Sharing a map and then inserting into the copy triggers exactly one full copy. All set algebra operations preserve correctness. Valgrind reports zero errors on all map/set COW test programs.
