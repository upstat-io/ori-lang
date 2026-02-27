---
section: "01"
title: "Runtime COW Foundation"
status: complete
goal: "Provide the runtime primitives that all COW collection operations depend on"
inspired_by:
  - "Swift stdlib/public/core/BridgeStorage.swift — isUniquelyReferenced pattern"
  - "Roc ori_rt refcount encoding — capacity vs refcount high-bit distinction"
  - "Lean 4 IR/ExpandResetReuse.lean — isShared instruction"
depends_on: []
sections:
  - id: "01.1"
    title: "Uniqueness Check API"
    status: complete
  - id: "01.2"
    title: "Capacity Management Primitives"
    status: complete
  - id: "01.3"
    title: "Growth Strategy"
    status: complete
  - id: "01.4"
    title: "Empty Collection Sentinels"
    status: complete
  - id: "01.5"
    title: "LLVM Runtime Declarations"
    status: complete
  - id: "01.6"
    title: "Completion Checklist"
    status: complete
---

# Section 01: Runtime COW Foundation

**Status:** Complete (2026-02-26)
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

- [x] Add `ori_rc_is_unique` function (2026-02-26)

- [x] Add `ori_rc_is_unique_or_null` variant for sentinel-aware code (2026-02-26)

- [x] Add unit tests: (2026-02-26)
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

- [x] Add `ori_rc_realloc` — resizes an existing RC allocation (2026-02-26)

- [x] Add `ori_memcpy_elements` — typed element copy (2026-02-26)

- [x] Add `ori_memmove_elements` — overlapping element move (for insert/remove) (2026-02-26)

- [x] Unit tests: (2026-02-26)
  - `ori_rc_realloc` with growth → data preserved, refcount preserved
  - `ori_rc_realloc` with shrink → data truncated correctly
  - `ori_memcpy_elements` → correct copy
  - `ori_memmove_elements` with overlap → no corruption

---

## 01.3 Growth Strategy

**File(s):** `compiler/ori_rt/src/lib.rs`

The growth strategy determines how much capacity to allocate when a collection runs out of space. This directly affects amortized performance.

**Design decision — growth factor:**

**(a) 2x doubling** (chosen — matches Rust Vec, Swift Array, Java ArrayList):
- Amortized O(1) append
- Wastes at most 50% capacity
- Simple, predictable, well-understood
- Memory: at most 2x the used size

- [x] Add growth factor constants and helper (`MIN_COLLECTION_CAPACITY = 4`, `next_capacity()`) (2026-02-26)

- [x] Add `ori_list_ensure_capacity` — grows a list's buffer if needed (2026-02-26)

- [x] Unit tests: (2026-02-26)
  - `next_capacity(0, 1)` → `MIN_COLLECTION_CAPACITY` (4)
  - `next_capacity(4, 5)` → 8
  - `next_capacity(8, 9)` → 16
  - `next_capacity(8, 100)` → 100 (required > doubled)
  - Overflow: `next_capacity(usize::MAX / 2 + 1, 1)` → saturates, doesn't panic

---

## 01.4 Empty Collection Sentinels

**File(s):** `compiler/ori_rt/src/lib.rs`

Empty collections should not allocate. Instead, they use a null data pointer sentinel. This eliminates heap allocation for the very common case of `[]`, `""`, `{}`, `#{}`.

**Design: Null-pointer sentinel** (chosen — matches current Ori pattern):
- `OriList { len: 0, cap: 0, data: null }`
- `OriStr { len: 0, data: null }`
- `OriMap { len: 0, cap: 0, keys: null, values: null }`
- `OriSet { len: 0, cap: 0, data: null }`

- [x] Verify `ori_rc_inc` and `ori_rc_dec` handle null data pointers (no-op) (2026-02-26)

- [x] Add `ori_list_empty` constructor (2026-02-26)

- [x] Add `ori_str_empty` constructor (2026-02-26)

- [x] Add `ori_map_empty` and `ori_set_empty` constructors (same pattern) (2026-02-26)

- [x] Update all runtime functions that create empty collections to use sentinels (2026-02-26)
  - Verified: `OriStr::from_owned` already returns null for empty strings
  - Verified: `ori_list_new` already uses null for zero-capacity lists

- [x] Unit tests: (2026-02-26)
  - Empty list sentinel → `len == 0`, `cap == 0`, `data == null`
  - `ori_rc_is_unique(null)` → false
  - `ori_rc_inc(null)` → no-op, no crash
  - `ori_rc_dec(null, drop_fn)` → no-op, drop_fn not called
  - Push to empty list → allocates new buffer (sentinel is not mutated)

---

## 01.5 LLVM Runtime Declarations

**File(s):** `compiler/ori_llvm/src/codegen/runtime_decl/runtime_functions.rs`, `compiler/ori_llvm/src/evaluator/runtime_mappings.rs`

All new runtime functions declared in the LLVM codegen with correct signatures and attributes. Also added `Ty::Map` variant for the `{i64, i64, ptr, ptr}` map representation.

- [x] Add `ori_rc_is_unique` declaration (2026-02-26)

- [x] Add `ori_rc_realloc` declaration (2026-02-26)

- [x] Add `ori_memcpy_elements` declaration (2026-02-26)

- [x] Add `ori_memmove_elements` declaration (2026-02-26)

- [x] Add `ori_list_ensure_capacity` declaration (2026-02-26)

- [x] Add sentinel constructor declarations (`ori_list_empty`, `ori_str_empty`, `ori_map_empty`, `ori_set_empty`) (2026-02-26)

- [x] Update `runtime_decl/tests.rs` to verify all new declarations link correctly (2026-02-26)
  - `declared_functions_covered_by_jit_or_aot_only` sync test passes (all 10 new functions registered in AOT_ONLY list)

---

## 01.6 Completion Checklist

- [x] `ori_rc_is_unique()` works correctly for RC=1, RC>1, and null pointers
- [x] `ori_rc_realloc()` preserves data and refcount across reallocation
- [x] `next_capacity()` implements 2x doubling with MIN_COLLECTION_CAPACITY=4
- [x] `ori_list_ensure_capacity()` grows a unique list's buffer
- [x] Empty sentinels work (null data pointer, no allocation)
- [x] All new functions declared in LLVM runtime_decl
- [x] All unit tests pass: `cargo test -p ori_rt` (87 tests)
- [x] AOT integration test: existing tests pass — runtime declarations verified via `declared_functions_covered_by_jit_or_aot_only` sync test
- [x] `cargo test --workspace` green (all workspace tests pass, zero failures)
- [x] `cargo clippy --workspace` green (zero warnings)

**Exit Criteria:** `ori_rc_is_unique()` is callable from LLVM-generated code. A test program that calls `ori_rc_is_unique()` and branches on the result compiles and runs correctly in AOT mode. All existing tests pass without modification.

---

## Reverification Audit (2026-02-27)

Full audit of all 33 checked items against actual codebase. Three findings discovered and resolved:

### Finding 1: Dead `Ty::List` removed prematurely
`Ty::List` was removed from `runtime_functions.rs` as dead code during §01, but `ori_args_from_argv` still returns `{i64, i64, ptr}` (the List struct). **Fix**: Re-added `Ty::List` variant.

### Finding 2: LLVM `runtime_tests.rs` stale
Test file referenced the boxed `ori_list_new` API (returns `*mut u8`) when the actual function still uses the old Box-based API (returns `*mut OriList`). **Fix**: Restored to HEAD.

### Finding 3: Premature function signature migration (6 functions reverted)
During §01 implementation, 6 pre-existing functions were migrated to a "boxed model" API, but LLVM codegen emitters still emit the old inline-struct calling convention. This caused 6 AOT test failures ("incorrect number of arguments"). **Root cause**: The codegen migration belongs in §02.7, not §01.

**Reverted to old inline-struct API** (keeping all new COW primitives):
- `ori_list_new` — Box-based `*mut OriList`, not RC-boxed `*mut u8`
- `ori_list_free` — `Box::from_raw()` cleanup, not `ori_free`+`ori_rc_free`
- `ori_list_push_new` — `(data, len, elem_ptr, elem_size, out_ptr) -> void`
- `ori_list_concat` — `(data1, len1, data2, len2, elem_size, out_ptr) -> void`
- `ori_list_reverse` — `(data, len, elem_size, out_ptr) -> void`
- `ori_args_from_argv` — returns `OriList` by value

**Kept** (all new §01 primitives):
- `ori_rc_is_unique`, `ori_rc_is_unique_or_null`
- `ori_rc_realloc`, `ori_memcpy_elements`, `ori_memmove_elements`
- `ori_list_ensure_capacity`, `ori_list_box_new`
- `ori_list_empty`, `ori_str_empty`, `ori_map_empty`, `ori_set_empty`
- `next_capacity()`, `MIN_COLLECTION_CAPACITY`, `OriMap` struct, `Ty::Map`

**Test result**: 10703 pass, 0 fail (87 ori_rt tests, 1016 LLVM tests).

### Additional primitives not listed in checkboxes
Two §01 functions were added but not tracked as explicit items:
- `ori_rc_is_unique_or_null` — sentinel-aware uniqueness check (in §01.1)
- `ori_list_box_new` — RC-allocates `OriList` metadata struct (foundation for §02.7 full migration)
