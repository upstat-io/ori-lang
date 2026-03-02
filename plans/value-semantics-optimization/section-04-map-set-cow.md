---
section: "04"
title: "Map & Set COW Operations"
status: complete
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
    status: complete
  - id: "04.3"
    title: "COW Map Remove & Update"
    status: complete
  - id: "04.4"
    title: "COW Set Operations"
    status: complete
  - id: "04.5"
    title: "Set Algebra (Union, Intersection, Difference)"
    status: complete
  - id: "04.6"
    title: "LLVM Codegen Updates"
    status: complete
  - id: "04.7"
    title: "Completion Checklist"
    status: complete
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

- [x] Replace `ori_map_insert` with COW-aware version:
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

- [x] **Key equality function**: Map insert needs to compare keys. The codegen passes a function pointer for key equality (type-specific). This matches the existing pattern for sort comparators.

- [x] Unit tests:
  - Insert into empty map → allocates
  - Insert new key into unique map with capacity → in-place
  - Insert existing key into unique map → overwrite in place
  - Insert into shared map → copy, original unchanged
  - Insert 1000 entries sequentially → amortized O(1)

---

## 04.3 COW Map Remove & Update

**File(s):** `compiler/ori_rt/src/lib.rs`

- [x] Add COW `ori_map_remove_cow`: (2026-02-28)
  - Unique + key found: shift keys and values left in place (overlapping `ptr::copy`)
  - Unique + last entry: `ori_rc_free` buffer, return empty sentinel
  - Shared + key found: alloc new buffer, copy all except removed, inc RC for copied elements
  - Key not found: return input unchanged (no-op)
  - 8 tests: not-found, unique-middle/first/last, unique-last-entry-frees, shared-copies, shared-last-entry-decs, empty

- [x] Add COW `ori_map_update_cow`: (2026-02-28)
  - Reuses `cow_insert_existing` helper — overwrite value if key found, no-op if not
  - Same consuming semantics and RC handling as insert
  - 4 tests: key-not-found, unique-overwrites, shared-copies, empty

- [x] Unit tests for remove and update with unique and shared maps (2026-02-28)

---

## 04.4 COW Set Operations

**File(s):** `compiler/ori_rt/src/lib.rs`

Sets follow the same COW pattern as maps but with simpler element handling (keys only, no values).

- [x] Replace `ori_set_insert` with COW version (`ori_set_insert_cow`): (2026-02-28)
  - Element exists: no-op, return input unchanged
  - Unique + has capacity: append in place
  - Unique + needs growth: realloc + append
  - Shared/empty: alloc new, copy all, append, dec old RC
  - Uses `elem_eq` function pointer (same pattern as map's `key_eq`)
  - 5 tests: empty, unique-new, existing-noop, shared-copies, unique-growth

- [x] Replace `ori_set_remove` with COW version (`ori_set_remove_cow`): (2026-02-28)
  - Not found: no-op, return input unchanged
  - Unique: shift left in place (overlapping `ptr::copy`)
  - Unique + last entry: `ori_rc_free`, return empty sentinel
  - Shared: alloc new, copy all except removed, dec old RC
  - 5 tests: not-found, unique-middle, unique-last-entry-frees, shared-copies, empty

- [x] Unit tests for insert and remove with unique and shared sets (2026-02-28)
  - Also modularized `set.rs` → `set/mod.rs` + `set/cow.rs` (matching map pattern)

---

## 04.5 Set Algebra (Union, Intersection, Difference)

**File(s):** `compiler/ori_rt/src/lib.rs`

Set algebra operations can benefit from COW when one operand is unique.

- [x] Replace `ori_set_union` with COW version (`ori_set_union_cow`): (2026-02-28)
  - Fast path (unique): extends in place, reallocs if needed
  - Slow path (shared): allocates new combined buffer, decs old RC
  - Identity law: A∪∅ returns unchanged, ∅∪B copies B
  - Superset no-op: pre-scans for new elements, returns unchanged if A⊇B

- [x] Replace `ori_set_intersection` with COW version (`ori_set_intersection_cow`): (2026-02-28)
  - Fast path (unique): compacts in place with write cursor
  - Slow path (shared): allocates new buffer with shared elements
  - Empty result: frees/decs buffer, returns empty sentinel

- [x] Replace `ori_set_difference` with COW version (`ori_set_difference_cow`): (2026-02-28)
  - Fast path (unique): compacts in place, same write-cursor pattern as intersection
  - Slow path (shared): allocates new buffer, copies remaining elements
  - Identity: A\∅ returns unchanged, ∅\B returns empty

- [x] Unit tests: (2026-02-28)
  - 18 tests: 7 union, 5 intersection, 6 difference
  - Union of unique sets → in-place extension
  - Union of shared set → new allocation
  - Intersection with unique → in-place compaction
  - Difference with unique → in-place compaction
  - Empty set operations (identity laws: A∪∅=A, A∩∅=∅, A\∅=A)
  - Self operations (A∪A=A, A∩A=A, A\A=∅)
  - Disjoint sets, superset no-op, growth path

---

## 04.6 LLVM Codegen Updates

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/builtins/collections.rs`

- [x] Update all map method emitters to call new COW runtime functions (2026-02-28)
  - `emit_map_insert` → `ori_map_insert_cow` with 11 args (data, len, cap, key_ptr, val_ptr, key_size, val_size, key_eq, key_inc, val_inc, out)
  - `emit_map_remove` → `ori_map_remove_cow` with 10 args (data, len, cap, key_ptr, key_size, val_size, key_eq, key_inc, val_inc, out)
  - Updated dispatch table: `borrow: true` → `borrow: false` for `("map", "insert")` and `("map", "remove")`
- [x] Update all set method emitters to call new COW runtime functions (2026-02-28)
  - `emit_set_insert` → `ori_set_insert_cow` with 9 args
  - `emit_set_remove` → `ori_set_remove_cow` with same signature
  - `emit_set_binary_op` → `ori_set_union_cow`/`intersection_cow`/`difference_cow` with 10 args
  - Updated dispatch table: `borrow: true` → `borrow: false` for 5 Set methods
- [x] Pass `key_eq` / `elem_eq` function pointers from codegen: (2026-02-28)
  - For primitive types (int, float, bool, char, byte, Duration, Size): generated inline `_ori_eq_{type}` functions with `icmp eq`/`fcmp oeq`
  - For string keys: passes `ori_str_eq` directly (same `fn(*const u8, *const u8) -> bool` signature)
  - Cached per element type via `eq_thunk_cache` on `ArcIrEmitter`
- [x] Update `OriMap` LLVM type if layout changes (single allocation) (2026-02-28)
  - Type already matches: `{i64 len, i64 cap, ptr data}` — single-buffer layout from §04.1
- [x] ARC ownership annotations updated for COW map/set methods: (2026-02-28)
  - Added `CONSUMING_RECEIVER_ONLY_METHOD_NAMES` for `remove`/`union`/`intersection`/`difference`
  - `compute_arg_ownership` produces `[Owned, Borrowed, ...]` for receiver-only-consuming methods
  - Prevents RC leaks on comparison keys and read-only collection args
  - All sync tests pass (`borrowing_builtins_sync_with_ori_arc`)
- [x] AOT integration tests for map and set operations (2026-02-28)
  - 21 map tests passing (insert, remove, get, contains_key, keys, values, size, is_empty with unique/shared/empty scenarios)
  - 10 set tests all passing — `__collect_set`, lexer union fix, and `check_collect_to_set` bidirectional unification fix (2026-03-01)
  - Fixed `ori_map_get`/`ori_map_contains_key` to pass `key_eq` callback for non-string key types
  - Fixed expand_reuse crash: collection constructors (List/Map/Set) now skipped in Reset/Reuse pairing
  - Fixed LLVM block ordering: RPO emission ensures definitions precede uses across expand_reuse-generated blocks

---

## 04.7 Completion Checklist

- [x] Map insert on unique map with capacity → in-place, same pointer (runtime tests)
- [x] Map insert on shared map → new allocation, original unchanged (runtime tests)
- [x] Map remove on unique map → in-place shift (runtime tests)
- [x] Map remove on shared map → new allocation (runtime tests)
- [x] Set insert/remove follow same COW pattern (runtime tests)
- [x] Union of unique set → in-place extension (runtime tests)
- [x] Intersection/difference of unique set → in-place shrink (runtime tests)
- [x] All set algebra identity laws pass (A∪∅=A, A∩∅=∅, etc.) (runtime tests)
- [x] Key equality handled correctly for all key types (eq thunk generation)
- [x] Element RC correct on COW paths (inc on copy, dec on remove) (runtime tests)
- [x] 1000-insert benchmark: O(N) total for unique map (runtime tests)
- [x] Valgrind clean on all map/set COW tests (2026-02-28)
  - All 6 Valgrind tests pass including `cow_map_operations.ori`
- [x] `./test-all.sh` green (10636 passed, 0 failed)
- [x] `./clippy-all.sh` green

**Exit Criteria:** A program that inserts 10,000 entries into a map completes in O(N) time when the map is uniquely owned. Sharing a map and then inserting into the copy triggers exactly one full copy. All set algebra operations preserve correctness. Valgrind reports zero errors on all map/set COW test programs.
