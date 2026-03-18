---
section: "01"
title: "RC Header Extension"
status: not-started
goal: "Extend RC allocation header from 16 to 24 bytes, adding an elem_dec_fn slot that persists across all RC dec calls"
depends_on: []
reviewed: false
third_party_review:
  status: none
  updated: null
sections:
  - id: "01.1"
    title: "Header Layout Change"
    status: not-started
  - id: "01.2"
    title: "Allocation Functions"
    status: not-started
  - id: "01.3"
    title: "RC Dec Functions"
    status: not-started
  - id: "01.4"
    title: "Slice-Aware Functions"
    status: not-started
  - id: "01.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "01.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 01: RC Header Extension

**Status:** Not Started
**Goal:** Extend the RC allocation header to store `elem_dec_fn`, ensuring element cleanup happens regardless of which `ori_buffer_rc_dec` call reaches zero.

**Context:** The current RC header is 16 bytes: `[data_size: i64 | strong_count: i64]`. The new V4 header adds 8 bytes for the element destructor function pointer: `[data_size: i64 | elem_dec_fn: ptr | strong_count: i64]` = 24 bytes.

<!-- reviewed: CRITICAL accuracy fix — elem_dec_fn must go BETWEEN data_size and strong_count,
     NOT after strong_count. All existing RC operations (ori_rc_inc, ori_rc_dec, ori_rc_count,
     ori_rc_is_unique, ori_buffer_rc_dec) rely on strong_count being at data_ptr - 8. Placing
     elem_dec_fn after strong_count would break this invariant. -->

**Reference implementations:**
- **Swift** `HeapObject.h`: Stores type metadata (including destructor) in a 2-word header alongside refcount.
- **Lean 4** `lean_object`: 8-byte header with tag + RC, element count in adjacent memory.

---

## 01.1 Header Layout Change

**File(s):** `compiler/ori_rt/src/rc/mod.rs`

Change the header constant and document the new layout.

- [ ] Change `RC_HEADER_SIZE` from `16` to `24`
- [ ] Update the doc comment (currently "V3 layout: `[data_size: i64 | strong_count: i64 | data ...]`") to "V4 layout: `[data_size: i64 | elem_dec_fn: *const () | strong_count: i64 | data ...]`"
- [ ] Add a constant `ELEM_DEC_FN_OFFSET` = 8 (offset from base pointer to elem_dec_fn field) <!-- reviewed: accuracy fix — offset is 8 (base+8), not 16, because layout is [data_size(0) | elem_dec_fn(8) | strong_count(16) | data(24)] -->
- [ ] Add helper functions:
  - `fn store_elem_dec_fn(data: *mut u8, f: Option<extern "C" fn(*mut u8)>)` — writes `elem_dec_fn` at `data - 16` (since data is at base + 24, and elem_dec_fn is at base + 8 = data - 16) <!-- reviewed: accuracy fix — corrected offset from data-8 to data-16 -->
  - `fn load_elem_dec_fn(data: *mut u8) -> Option<extern "C" fn(*mut u8)>` — reads from `data - 16`
- [ ] Verify: `strong_count` remains at `data - 8` (base + 16 in V4) — all existing RC operations (`ori_rc_inc`, `ori_rc_dec`, `ori_rc_count`, `ori_rc_is_unique`) must continue working without changes to their pointer arithmetic <!-- reviewed: added critical invariant verification -->
- [ ] Verify: `IsShared` LLVM IR emission in `arc_emitter` uses GEP i8 with `-8` offset to reach refcount — this is correct in V4 since strong_count stays at data - 8. Test at `arc_emitter/tests.rs` line ~689 asserts this. No code change needed, but verify the test passes. <!-- reviewed: completeness fix — LLVM-side hardcoded offset must be verified -->
- [ ] Add unit tests for helper functions: round-trip store/load, NULL initial value, verify strong_count at data - 8 is unaffected

### Cleanup <!-- reviewed: hygiene fix -->

- [ ] **[STYLE]** `compiler/ori_rt/src/rc/mod.rs:45` — Remove decorative banner `// ── Reference Counting (V3: 16-byte header, data-pointer style) ──────────`. Replace with plain section comment `// Reference Counting` per hygiene rules (no decorative characters).
- [ ] **[STYLE]** `compiler/ori_rt/src/rc/mod.rs:95` — Remove decorative banner `// ── Core RC Functions ────────────────────────────────────────────────`. Replace with plain `// Core RC Functions`.
- [ ] **[STYLE]** `compiler/ori_rt/src/rc/debug.rs:20,136,221,340` — Remove 4 decorative banners (`// ── RC Event Tracing`, `// ── Leak Attribution`, `// ── Runtime Assertion Mode`, `// ── Leak Detection`). Replace with plain section comments.

---

## 01.2 Allocation Functions

**File(s):** `compiler/ori_rt/src/rc/allocate.rs`

Update all allocation functions to account for the larger header.

- [ ] `ori_rc_alloc`: Initialize `elem_dec_fn` field to NULL (zero) at `base + 8`. Move `strong_count` init from `base + 8` to `base + 16`. `data_size` write at `base + 0` stays the same. Data pointer returned is now `base + 24` (via `base.add(RC_HEADER_SIZE)`). <!-- reviewed: accuracy fix — elem_dec_fn goes at base+8, strong_count moves to base+16 -->
- [ ] `ori_rc_free`: Already uses `data_ptr.sub(RC_HEADER_SIZE)` and `size + RC_HEADER_SIZE` — no pointer arithmetic changes needed, just verify the updated constant propagates correctly. <!-- reviewed: accuracy fix — function uses RC_HEADER_SIZE constant, not hardcoded 16 -->
- [ ] `ori_rc_realloc`: Already uses `data_ptr.sub(RC_HEADER_SIZE)` and `RC_HEADER_SIZE` for total sizes — no pointer arithmetic changes needed. Verify the realloc preserves all 24 header bytes (data_size + strong_count + elem_dec_fn). <!-- reviewed: accuracy fix — function uses RC_HEADER_SIZE constant -->
- [ ] `ori_rc_data_size`: Uses `data_ptr.sub(RC_HEADER_SIZE).cast::<i64>()` — no code change needed since `RC_HEADER_SIZE` is updated from 16 to 24 in step 01.1. Verify the constant propagates correctly. <!-- reviewed: accuracy fix — function already uses RC_HEADER_SIZE constant, so the change is automatic -->
- [ ] Update all `// SAFETY:` comments that reference "16 bytes" to say "24 bytes"
- [ ] Update `compiler/ori_llvm/src/aot/debug/builder_scope.rs`: The `create_rc_heap_type` function (line ~327) has hardcoded DWARF debug info offsets for the V3 layout. Must add `elem_dec_fn` field at `offset_bits: 64`, move `strong_count` to `offset_bits: 128`, move `data` to `offset_bits: 192`, and update `total_size` from `128 + inner_size_bits` to `192 + inner_size_bits`. <!-- reviewed: added — hardcoded debug info offsets need updating -->
- [ ] Update `compiler/ori_llvm/src/tests/runtime_tests.rs`: The `test_rc_header_is_16_bytes` test (line ~289) hardcodes V3 offsets (`data_ptr - 8` = strong_count, `data_ptr - 16` = data_size). Must update to V4: `data_ptr - 8` = strong_count (unchanged), `data_ptr - 16` = elem_dec_fn (new), `data_ptr - 24` = data_size (moved), and rename to `test_rc_header_is_24_bytes`. <!-- reviewed: added — existing test will break -->
- [ ] Update `compiler/ori_rt/src/rc/allocate.rs` doc comments and SAFETY comments: lines 2, 4, 18, 23-25, 72-73, 77, 87-88, 107-108, 110, 139-141, 155, 182-183, 197-198 all reference "16 bytes" or "16-byte header" or "base + 16" — all must be updated to 24. <!-- reviewed: completeness fix — many hardcoded references -->
- [ ] Update `compiler/ori_rt/src/rc/allocate.rs` `ori_rc_alloc`: the `base.add(8)` for strong_count (line 46) must change to `base.add(16)`, and a new `base.add(8)` line must zero-initialize `elem_dec_fn`. <!-- reviewed: completeness fix — the actual pointer arithmetic in ori_rc_alloc is hardcoded, not using RC_HEADER_SIZE -->
- [ ] Update `compiler/ori_rt/src/list/reset/mod.rs` line 74: `data.sub(8).cast::<i64>()` — this reads strong_count and DOES remain correct (strong_count stays at data - 8). Add a comment confirming V4 compatibility. <!-- reviewed: completeness fix — verify this code path is unaffected -->
- [ ] Update `compiler/ori_rt/src/rc/debug.rs` line 244: `data_ptr.sub(8).cast::<i64>().read()` — reads strong_count, remains correct. Verify. <!-- reviewed: completeness fix -->
- [ ] Update `docs/compiler/design/11-runtime/data-structures.md`: references "16-byte header" and `RC_HEADER_SIZE = 16` at lines 23 and 53. Update to 24-byte header and document the new `elem_dec_fn` field. <!-- reviewed: completeness fix — documentation must be updated -->
- [ ] Update `docs/compiler/design/11-runtime/reference-counting.md`: references "16-byte header" at lines 28, 63, 79, 170, 303. Update layout diagrams and size references. <!-- reviewed: completeness fix — documentation must be updated -->
- [ ] Update `docs/compiler/design/11-runtime/index.md` line 48: references "16-byte header". <!-- reviewed: completeness fix -->
- [ ] Update `compiler/ori_rt/src/map/hash_table.rs` lines 10, 16: doc comments reference "RC header (16 bytes)". <!-- reviewed: completeness fix — hash table layout docs -->
- [ ] Add unit test: allocate, store elem_dec_fn, verify it's preserved after realloc

### Cleanup <!-- reviewed: hygiene fix -->

- [ ] **[WASTE]** `compiler/ori_rt/src/rc/mod.rs:45-67` — The 22-line block comment documenting V3 layout will become stale. When updating to V4, condense to a compact layout diagram (same information, fewer lines), removing the verbose prose that restates the code.
- [ ] **[STYLE]** `compiler/ori_rt/src/rc/allocate.rs:1-2` — Module doc says "V3 16-byte header layout (see `mod.rs` for diagram)". Must be updated to V4 and reference the new layout.

---

## 01.3 RC Dec Functions

**File(s):** `compiler/ori_rt/src/rc/mod.rs`, `compiler/ori_rt/src/rc/list_rc.rs`

Update `ori_buffer_rc_dec` and related functions to use the stored `elem_dec_fn`.

- [ ] `ori_buffer_rc_dec(data, len, cap, elem_size, elem_dec_fn)`: Before decrementing RC:
  - If `elem_dec_fn` is non-NULL and stored header value is NULL: write `elem_dec_fn` to header via `store_elem_dec_fn(data, elem_dec_fn)` (writes at `data - 16`)
  - When RC reaches zero: read `elem_dec_fn` from header via `load_elem_dec_fn(data)` (reads from `data - 16`) — use THIS value (not the parameter) for element iteration
  - This means even if the parameter is NULL, the stored function is used
- [ ] **Thread safety of write-once pattern**: The "first non-NULL wins" write to `elem_dec_fn` must be thread-safe. Use `AtomicPtr::compare_exchange(null, new_fn, Relaxed, Relaxed)` to ensure exactly one writer wins. A plain pointer write could race if two threads dec simultaneously. Since the function pointer is always the same value (all callers for the same buffer produce the same `elem_dec_fn`), the CAS only needs to handle the null→non-null transition. The `#[cfg(feature = "single-threaded")]` path can use a plain pointer write. <!-- reviewed: completeness fix — thread safety of the write-once pattern was not addressed -->
- [ ] `ori_buffer_drop_unique(data, len, cap, elem_size, elem_dec_fn)`: Same store-then-read pattern — store if non-NULL, read from header for cleanup
- [ ] `slice_buffer_rc_dec`: The slice stores the ORIGINAL buffer's data pointer. The `elem_dec_fn` should be stored on the ORIGINAL buffer's header, not the slice's. Trace through `slice_original_data()` to find the original, store on it. Note: `slice_original_data` computes the original data pointer, which has its own RC header — the `elem_dec_fn` is at `original_data - 16` in the corrected V4 layout.
- [ ] `ori_set_buffer_rc_dec` in `compiler/ori_rt/src/rc/set_rc.rs`: Same store-then-read pattern as `ori_buffer_rc_dec` — sets have a single `elem_dec_fn`. Also update `ori_set_buffer_drop_unique` in the same file. <!-- reviewed: completeness fix — added file path and drop_unique -->
- [ ] `ori_map_buffer_rc_dec` in `compiler/ori_rt/src/rc/map_rc.rs`: Maps take TWO cleanup functions (`key_dec_fn` and `val_dec_fn`). A single `elem_dec_fn` header slot is insufficient. **Decision required**: either (a) store a combined "map drop descriptor" pointer in the header — a heap-allocated `(key_dec_fn, val_dec_fn)` pair that the header points to (requires lifecycle management for the descriptor itself), (b) add a second header slot for maps only (32-byte header for maps, 24-byte for lists — but then `ori_rc_alloc` needs to know which type it's allocating for), or (c) use a different approach for maps: store the dec functions in the `IterState::Map` at construction time by changing `emit_map_iter` to pass real functions instead of NULL (this is the simplest option and is recommended — it avoids complicating the header). Also update `ori_map_buffer_drop_unique`. <!-- reviewed: accuracy fix — maps have key_dec_fn + val_dec_fn, not a single elem_dec_fn; added concrete recommendation -->
- [ ] **Recommended approach for maps (option c)**: Instead of extending the header, change `emit_map_iter` in `map_builtins.rs` to pass the real `key_dec_fn`/`val_dec_fn` (from `get_or_generate_elem_dec_fn`) instead of NULL. Then `IterState::Map` Drop passes them through to `ori_map_buffer_rc_dec`. This works because the `__for_coll` phantom binding already ensures correct ordering for maps (the comment in `loops.rs` line 173 says "Map" but the code at line 174 only matches `List | Set` — so maps rely on a different mechanism or are currently buggy). Investigate whether maps have the ordering issue in practice before implementing header-based cleanup. <!-- reviewed: completeness fix — concrete recommendation for maps -->
- [ ] Add unit tests:
  - Alloc buffer, call `ori_buffer_rc_dec(data, ..., real_fn)` then `ori_buffer_rc_dec(data, ..., NULL)` in sequence — verify element cleanup happens when NULL-carrying call reaches zero
  - Same test with reversed order — verify element cleanup also happens
  - Test with only NULL calls — no element cleanup (no stored function)
  - Test CAS on elem_dec_fn: two threads calling with the same non-NULL fn — both succeed, no corruption <!-- reviewed: completeness fix — thread safety test -->

---

## 01.4 Slice-Aware Functions

**File(s):** `compiler/ori_rt/src/rc/list_rc.rs`, `compiler/ori_rt/src/slice_encoding/mod.rs`

Ensure seamless slices correctly interact with the new header.

- [ ] `ori_list_rc_inc`: No code change needed — it calls `ori_rc_inc(rc_target)` which uses `data_ptr.sub(8)` for the refcount. Since strong_count remains at `data_ptr - 8` in V4, this continues to work. <!-- reviewed: accuracy fix — clarified why no change needed -->
- [ ] Verify `is_slice_cap(cap)` still works — SLICE_FLAG is in cap, not in the header
- [ ] Verify `slice_original_data(data, cap)` still correctly computes the original buffer's data pointer — the byte offset between slice data and original data is independent of header size (it's computed from the data pointer, not the base pointer). However, update the **doc comments** in `slice_encoding/mod.rs` that hardcode "16" (lines 11-12: "RC_HEADER_SIZE (16 bytes)" and line 64: "result - 16") to reference the constant instead. <!-- reviewed: accuracy fix — code is correct, but doc comments hardcode 16 -->
- [ ] Test: create a list, create a slice, call `ori_buffer_rc_dec` on the slice — verify the original buffer's elem_dec_fn is used for element cleanup

---

## 01.R Third Party Review Findings

- None.

---

## 01.N Completion Checklist

- [ ] `RC_HEADER_SIZE == 24` in `compiler/ori_rt/src/rc/mod.rs`
- [ ] All allocation functions (`ori_rc_alloc`, `ori_rc_free`, `ori_rc_realloc`) use 24-byte header
- [ ] `ori_rc_alloc` zero-initializes `elem_dec_fn` at `base + 8` and writes `strong_count` at `base + 16`
- [ ] `ori_buffer_rc_dec` stores non-NULL elem_dec_fn in header and reads from header at cleanup time
- [ ] `ori_set_buffer_rc_dec` uses same store-then-read pattern
- [ ] Map strategy decided and implemented (header-based or codegen-based)
- [ ] `store_elem_dec_fn`/`load_elem_dec_fn` use atomic CAS for thread safety (non-single-threaded path) <!-- reviewed: completeness fix -->
- [ ] DWARF debug info in `create_rc_heap_type` updated for V4 layout
- [ ] `test_rc_header_is_16_bytes` renamed and updated for V4
- [ ] All hardcoded "16" references in doc comments updated to 24 (allocate.rs, slice_encoding, hash_table.rs, docs/) <!-- reviewed: completeness fix -->
- [ ] All `data_ptr.sub(8)` for strong_count confirmed still correct (spot-check list_rc.rs, map_rc.rs, set_rc.rs, mod.rs, debug.rs, reset/mod.rs, tests.rs) <!-- reviewed: completeness fix -->
- [ ] All RC runtime tests pass (`timeout 150 cargo test -p ori_rt`)
- [ ] No regressions in AOT tests (`timeout 150 cargo test -p ori_llvm --test aot`)
- [ ] Valgrind clean on existing heap-allocating tests
