---
section: "01"
title: "RC Header Extension"
status: in-progress
goal: "Extend RC allocation header to 32 bytes (V5), adding elem_dec_fn and elem_count slots for reliable element cleanup"
depends_on: []
reviewed: true
third_party_review:
  status: resolved
  updated: 2026-03-20
sections:
  - id: "01.1"
    title: "Header Layout Change"
    status: complete
  - id: "01.2"
    title: "Allocation Functions"
    status: complete
  - id: "01.3"
    title: "RC Dec Functions"
    status: complete
  - id: "01.4"
    title: "Slice-Aware Functions"
    status: complete
    # V5 elem_count fix + semantic pin tests + Valgrind/AOT test all complete
  - id: "01.R"
    title: "Third Party Review Findings"
    status: complete
  - id: "01.N"
    title: "Completion Checklist"
    status: in-progress
---

# Section 01: RC Header Extension

**Status:** In Progress
**Goal:** Extend the RC allocation header to store `elem_dec_fn`, ensuring element cleanup happens regardless of which `ori_buffer_rc_dec` call reaches zero.

**Context:** The current RC header is 16 bytes: `[data_size: i64 | strong_count: i64]`. The new V4 header adds 8 bytes for the element destructor function pointer: `[data_size: i64 | elem_dec_fn: ptr | strong_count: i64]` = 24 bytes. The `elem_dec_fn` slot MUST go between `data_size` and `strong_count` so that `strong_count` remains at `data_ptr - 8` — all existing RC operations (`ori_rc_inc`, `ori_rc_dec`, `ori_rc_count`, `ori_rc_is_unique`, `ori_buffer_rc_dec`) rely on this invariant.

**Reference implementations:**
- **Swift** `HeapObject.h`: Stores type metadata (including destructor) in a 2-word header alongside refcount.
- **Lean 4** `lean_object`: 8-byte header with tag + RC, element count in adjacent memory.

---

## 01.1 Header Layout Change

**File(s):** `compiler/ori_rt/src/rc/mod.rs`

Change the header constant and document the new layout.

**V4 offset reference** (all offsets in bytes):

| Field | From base | From data_ptr |
|-------|-----------|---------------|
| `data_size` | base + 0 | data - 24 |
| `elem_dec_fn` | base + 8 | data - 16 |
| `strong_count` | base + 16 | data - 8 |
| `data` | base + 24 | data + 0 |

- [x] Change `RC_HEADER_SIZE` from `16` to `24`
- [x] Update the doc comment on `RC_HEADER_SIZE` (line 97-98, currently "V3 layout: ... The header is 16 bytes: 8 for `data_size` + 8 for `strong_count`.") to "V4 layout: `[data_size: i64 | elem_dec_fn: *const () | strong_count: i64 | data ...]` The header is 24 bytes: 8 for `data_size` + 8 for `elem_dec_fn` + 8 for `strong_count`."
- [x] Update `ori_rc_inc` doc comment (line 101-108): references `strong_count` at `data_ptr - 8` — still correct in V4, but add a note that `data_ptr - 16` is now `elem_dec_fn` (not `data_size`)
- [x] Add a constant `ELEM_DEC_FN_OFFSET` = 16 (byte offset from data pointer to `elem_dec_fn` field, i.e., `data_ptr.sub(ELEM_DEC_FN_OFFSET)` reaches `elem_dec_fn` at base + 8)
- [x] Add helper functions:
  - `fn store_elem_dec_fn(data: *mut u8, f: Option<extern "C" fn(*mut u8)>)` — writes `elem_dec_fn` at `data - 16` (data is at base + 24, elem_dec_fn is at base + 8, so offset = 16)
  - `fn load_elem_dec_fn(data: *mut u8) -> Option<extern "C" fn(*mut u8)>` — reads from `data - 16`
- [x] Verify: `strong_count` remains at `data - 8` (base + 16 in V4) — all existing RC operations (`ori_rc_inc`, `ori_rc_dec`, `ori_rc_count`, `ori_rc_is_unique`) must continue working without changes to their pointer arithmetic
- [x] Verify: `IsShared` LLVM IR emission in `arc_emitter` uses GEP i8 with `-8` offset to reach refcount — this is correct in V4 since strong_count stays at data - 8. Test at `arc_emitter/tests.rs` line ~689 asserts this. No code change needed, but verify the test passes.
- [x] Add unit tests for helper functions: round-trip store/load, NULL initial value, verify strong_count at data - 8 is unaffected (covered by test_rc_header_is_24_bytes in runtime_tests.rs)

### Cleanup

- [x] **[STYLE]** `compiler/ori_rt/src/rc/mod.rs:43` — Remove decorative banner `// ── Reference Counting (V3: 16-byte header, data-pointer style) ──────────`. Replace with plain section comment `// Reference Counting` per hygiene rules (no decorative characters).
- [x] **[STYLE]** `compiler/ori_rt/src/rc/mod.rs:93` — Remove decorative banner `// ── Core RC Functions ────────────────────────────────────────────────`. Replace with plain `// Core RC Functions`.
- [x] **[STYLE]** `compiler/ori_rt/src/rc/debug.rs:20,136,221,340` — Remove 4 decorative banners (`// ── RC Event Tracing`, `// ── Leak Attribution`, `// ── Runtime Assertion Mode`, `// ── Leak Detection`). Replace with plain section comments.
- [x] **[STYLE]** `compiler/ori_rt/src/map/hash_table.rs:39,93,113,180,211,296` — Remove 6 decorative banners (`// ── Layout Computation`, `// ── Metadata Operations`, `// ── Probing`, `// ── Capacity Management`, `// ── Rehashing`, `// ── Counting`). Replace with plain section comments.

---

## 01.2 Allocation Functions

**File(s):** `compiler/ori_rt/src/rc/allocate.rs`

Update all allocation functions to account for the larger header.

- [x] `ori_rc_alloc`: Initialize `elem_dec_fn` field to NULL (zero) at `base + 8`. Move `strong_count` init from `base + 8` (line 46: `base.add(8)`) to `base + 16` (change to `base.add(16)`). Add a new `base.add(8)` line to zero-initialize `elem_dec_fn`. `data_size` write at `base + 0` stays the same. Data pointer returned is now `base + 24` (via `base.add(RC_HEADER_SIZE)` — automatic since `RC_HEADER_SIZE` changes to 24).
- [x] `ori_rc_free`: Already uses `data_ptr.sub(RC_HEADER_SIZE)` and `size + RC_HEADER_SIZE` — no pointer arithmetic changes needed, just verify the updated constant propagates correctly.
- [x] `ori_rc_realloc`: Already uses `data_ptr.sub(RC_HEADER_SIZE)` and `RC_HEADER_SIZE` for total sizes — no pointer arithmetic changes needed. Verify the realloc preserves all 24 header bytes (data_size + elem_dec_fn + strong_count). Added SAFETY comment for elem_dec_fn preservation.
- [x] `ori_rc_data_size`: Uses `data_ptr.sub(RC_HEADER_SIZE).cast::<i64>()` — no code change needed since `RC_HEADER_SIZE` is updated from 16 to 24 in step 01.1. Verify the constant propagates correctly.
- [x] Update all `// SAFETY:` comments that reference "16 bytes" to say "24 bytes"
- [x] Update `compiler/ori_llvm/src/aot/debug/builder_scope.rs`: The `create_rc_heap_type` function (line ~327) has hardcoded DWARF debug info offsets for the V3 layout. Must add `elem_dec_fn` field at `offset_bits: 64`, move `strong_count` to `offset_bits: 128`, move `data` to `offset_bits: 192`, and update `total_size` from `128 + inner_size_bits` to `192 + inner_size_bits`.
- [x] Update `compiler/ori_llvm/src/tests/runtime_tests.rs`: The `test_rc_header_is_16_bytes` test renamed to `test_rc_header_is_24_bytes` and updated for V4: `data_ptr - 8` = strong_count (unchanged), `data_ptr - 16` = elem_dec_fn (new, verify NULL), `data_ptr - 24` = data_size (moved). Includes `ori_rc_data_size()` verification and elem_dec_fn NULL verification + realloc preservation.
- [x] Update `compiler/ori_rt/src/tests.rs`: Tests `rc_inc_skips_at_max_refcount` and `rc_dec_skips_at_max_refcount` use `ptr.sub(8)` for strong_count — these are CORRECT and unchanged in V4, verified they pass.
- [x] Update `compiler/ori_rt/src/rc/allocate.rs` doc comments and SAFETY comments: all references to "16 bytes" or "16-byte header" updated to 24.
- [x] Update `compiler/ori_rt/src/list/reset/mod.rs`: `is_unique` replaced with `ori_rc_is_unique` call (data race fix).
- [x] Update `compiler/ori_rt/src/rc/debug.rs` line 244: `data_ptr.sub(8).cast::<i64>().read()` — reads strong_count, remains correct. Verified.
- [x] Update `docs/compiler/design/11-runtime/data-structures.md`: references updated for V4 header size, layout diagrams, offset table, and mermaid diagram. New `elem_dec_fn` field documented.
- [x] Update `docs/compiler/design/11-runtime/reference-counting.md`: layout diagrams, struct definitions, offset references, and size references updated for V4.
- [x] Update `docs/compiler/design/11-runtime/index.md`: references updated for V4 header. Fixed backwards offset description.
- [x] Update `compiler/ori_rt/src/map/hash_table.rs` lines 10, 16: doc comments updated to "24 bytes".
- [x] Update `docs/compiler/design/11-runtime/string-sso.md`: references updated to "24-byte RC header".
- [x] Update `docs/compiler/design/11-runtime/collections-cow.md`: references updated to 24.
- [x] Add unit test: allocate, store elem_dec_fn, verify it's preserved after realloc (covered by test_rc_header_is_24_bytes in runtime_tests.rs)
- [x] **Verify `ori_rc_realloc` preserves `elem_dec_fn`**: verified and documented with SAFETY comment.

**Forward dependency for Section 02**: The header change in Section 01 only creates the infrastructure (`store_elem_dec_fn`/`load_elem_dec_fn`) and updates existing `ori_buffer_rc_dec` to read from the header. Section 02 must ensure `elem_dec_fn` is STORED in the header for every buffer that contains fat-pointer elements. This includes not just codegen-level list construction, but also ALL runtime functions that create new list buffers via `ori_rc_alloc`:

| File | Functions | Notes |
|------|-----------|-------|
| `list/cow.rs` | `ori_list_push_cow`, `ori_list_insert_cow`, `ori_list_reverse_cow` | Slow path creates new buffer |
| `list/cow_structural.rs` | `ori_list_concat_cow`, `ori_list_flatten_cow` | Creates new buffer for result |
| `list/cow_sort.rs` | `ori_list_sort_cow`, `ori_list_sort_by_cow`, merge sort helpers | Multiple `ori_rc_alloc` calls |
| `list/query.rs` | `ori_list_filter`, `ori_list_slice_copy` | Creates filtered/copied buffers |
| `list/slice.rs` | `ori_list_slice_materialized` | Materializes slice into owned buffer |
| `list/mod.rs` | `ori_list_alloc_data`, `ori_list_new_with_data`, `ori_list_grow`, etc. | Core allocation paths |
| `list/reset/mod.rs` | `ori_list_reset_buffer` | Creates new buffer when reuse fails |
| `iterator/consumers.rs` | `ori_iter_collect` | Creates output buffer for collect |
| `set/mod.rs` | Set allocation functions | Creates set hash table buffers |
| `map/mod.rs`, `map/hash_table.rs` | Map allocation functions | Creates map hash table buffers |

Each of these must propagate `elem_dec_fn` to the new buffer. The two approaches are: (a) the runtime function reads `elem_dec_fn` from the OLD buffer's header and writes it to the NEW buffer's header (recommended for COW functions that have access to the old buffer), or (b) the codegen always passes real `elem_dec_fn` to `ori_buffer_rc_dec` for the new buffer's type, and the first non-NULL write populates the header (works for `collect` and `alloc_data` where there is no "old buffer"). Section 02 must enumerate and handle each case.

### Cleanup

- [x] **[WASTE]** `compiler/ori_rt/src/rc/mod.rs:43-68` — V3 layout comment condensed to compact V4 layout diagram.
- [x] **[STYLE]** `compiler/ori_rt/src/rc/allocate.rs:1-4` — Module doc updated to V4 and reference the new layout.
- [x] **[WASTE]** `compiler/ori_rt/src/rc/allocate.rs:168-174` — `rc_trace_realloc()` helper added to `debug.rs` and called from `ori_rc_realloc`.
- [x] **[WASTE]** `compiler/ori_rt/src/list/reset/mod.rs:69-76` — `is_unique()` replaced with `ori_rc_is_unique()` call (data race fix).
- [x] **[WASTE]** `compiler/ori_rt/src/rc/map_rc.rs:92` — `_len` parameter removed from `map_buffer_cleanup`.

---

## 01.3 RC Dec Functions

**File(s):** `compiler/ori_rt/src/rc/mod.rs`, `compiler/ori_rt/src/rc/list_rc.rs`

Update `ori_buffer_rc_dec` and related functions to use the stored `elem_dec_fn`.

- [x] `ori_buffer_rc_dec(data, len, cap, elem_size, elem_dec_fn)`: store_elem_dec_fn_once before dec, drop_elements_and_free reads from header via load_elem_dec_fn.
- [x] **Both `#[cfg]` branches must be updated**: Both branches updated for list_rc, set_rc. Store elem_dec_fn in header before dec, read from header in drop path.
- [x] **Thread safety of write-once pattern**: store_elem_dec_fn_once uses AtomicPtr CAS on non-single-threaded path. Plain pointer write on single-threaded path.
- [x] `ori_buffer_drop_unique(data, len, cap, elem_size, elem_dec_fn)`: Stores to header, reads from header for cleanup.
- [x] `slice_buffer_rc_dec`: Stores/reads on ORIGINAL buffer's header (not slice data).
- [x] `ori_set_buffer_rc_dec` in `compiler/ori_rt/src/rc/set_rc.rs`: Store-then-read pattern via set_buffer_cleanup. Both `#[cfg]` branches and `ori_set_buffer_drop_unique` updated.
- [x] `ori_map_buffer_rc_dec` in `compiler/ori_rt/src/rc/map_rc.rs`: **Decision: option (c) — codegen-based, not header-based.** Maps keep using parameters (two cleanup functions cannot fit in single header slot). Recommended approach documented.
- [x] **Recommended approach for maps (option c)**: Documented in plan. Maps use codegen-based approach, not header-based.
- [x] Add unit tests:
  - [x] `semantic_pin_header_based_element_cleanup`: RC 2→1 with real fn, then 1→0 with NULL — header fn used
  - [x] `elem_dec_fn_null_initialized`: alloc → immediate load → NULL
  - [x] `elem_dec_fn_store_and_load_roundtrip`: store non-NULL → load matches
  - [x] `elem_dec_fn_store_once_first_non_null_wins`: first wins, second is no-op
  - [x] `elem_dec_fn_store_once_null_is_noop`: store_once(None) doesn't change NULL
  - [x] `elem_dec_fn_preserved_through_realloc`: store → realloc → load matches
  - [x] `drop_unique_reads_from_header`: pre-stored fn, call with NULL param → header fn used

---

## 01.4 Slice-Aware Functions

**File(s):** `compiler/ori_rt/src/rc/list_rc.rs`, `compiler/ori_rt/src/slice_encoding/mod.rs`

Ensure seamless slices correctly interact with the new header.

- [x] `ori_list_rc_inc`: Verified no code change needed — strong_count remains at `data_ptr - 8` in V4.
- [x] Verify `is_slice_cap(cap)` still works — SLICE_FLAG is in cap, not in the header. Verified.
- [x] Verify `slice_original_data(data, cap)` still correctly computes the original buffer's data pointer. Doc comments updated to reference `RC_HEADER_SIZE` instead of hardcoded "16".
- [x] Test: create a list, create a slice, call `ori_buffer_rc_dec` on the slice — verify the original buffer's elem_dec_fn is used for element cleanup (`slice_uses_original_buffers_elem_dec_fn`)
- [x] **[TPR-01-001]** Fix `slice_buffer_rc_dec` to clean up ALL initialized elements when slice is last owner — V5 header with `elem_count` field. (2026-03-20)
  - [x] Add `ELEM_COUNT_OFFSET` constant (byte offset from data_ptr to `elem_count` field)
  - [x] Add `store_elem_count`/`load_elem_count` helpers
  - [x] Expand `RC_HEADER_SIZE` from 24 to 32 bytes, update header layout
  - [x] Update `ori_rc_alloc` to zero-initialize `elem_count` and adjust offsets
  - [x] Update `ori_rc_free`, `ori_rc_realloc`, `ori_rc_data_size` for 32-byte header
  - [x] Update `ori_buffer_rc_dec` to store caller's `len` as `elem_count` in header (non-slice path)
  - [x] Update `slice_buffer_rc_dec` to read `elem_count` from header and iterate from `original_data` for `elem_count` elements
  - [x] Update DWARF debug info in `create_rc_heap_type` for V5 layout
  - [x] Update all doc comments referencing "24 bytes" to "32 bytes"
  - [x] Add semantic pin test: slice outliving original, verify all elements cleaned up (4 tests: middle, end, empty, full-range)
  - [x] Valgrind/AOT test: `tests/valgrind/slice_str_outlives_original.ori` — 5 patterns (middle, end, beginning, empty, single-element slice), all Valgrind clean + `ORI_CHECK_LEAKS=1` clean (2026-03-20)

---

## 01.R Third Party Review Findings

- [x] `[TPR-01-001][high]` `compiler/ori_rt/src/rc/list_rc.rs:157` — `slice_buffer_rc_dec` still leaks child RCs for elements outside the visible slice when the slice is the last owner.
  Resolved: Validated on 2026-03-20. Accepted — real issue. `drop_elements_and_free` uses `slice_len` (not original buffer's element count) so elements outside the slice range leak their RC children. Implementation task added to 01.4: extend header with `elem_count` field for full-range cleanup.
- [x] `[TPR-01-002][medium]` `plans/rc-header-elem-dec/section-01-rc-header.md:203` — Section 01 marks verification complete even though the recorded valgrind result still has five failing cases waived as "pre-existing" and "unrelated."
  Resolved: Validated on 2026-03-20. Accepted — all 5 failures confirmed still present (invalid reads/frees, not just leaks). Completion item un-checked and reworded. 5 COW/TRMC valgrind failures added as open items in 01.N.

---

## 01.N Completion Checklist

- [x] `RC_HEADER_SIZE == 32` in `compiler/ori_rt/src/rc/mod.rs` (V5, was 24 in V4)
- [x] `ELEM_DEC_FN_OFFSET == 24` constant (byte offset from data_ptr to elem_dec_fn, was 16 in V4)
- [x] `ELEM_COUNT_OFFSET == 16` constant added (V5: byte offset from data_ptr to elem_count)
- [x] All allocation functions (`ori_rc_alloc`, `ori_rc_free`, `ori_rc_realloc`) use 32-byte header
- [x] `ori_rc_alloc` zero-initializes `elem_dec_fn` at `base + 8`, `elem_count` at `base + 16`, and writes `strong_count` at `base + 24`
- [x] `ori_buffer_rc_dec` stores non-NULL elem_dec_fn in header and reads from header at cleanup time
- [x] `ori_buffer_drop_unique` reads `elem_dec_fn` from header (not parameter)
- [x] `slice_buffer_rc_dec` stores/reads on ORIGINAL buffer's header (not slice data)
- [x] `ori_set_buffer_rc_dec` and `ori_set_buffer_drop_unique` use same store-then-read pattern
- [x] Map strategy decided and implemented (codegen-based — option c)
- [x] `store_elem_dec_fn`/`load_elem_dec_fn` use atomic CAS for thread safety (non-single-threaded path)
- [x] All dual `#[cfg]` branches updated (non-single-threaded + single-threaded) in list_rc.rs, set_rc.rs
- [x] DWARF debug info in `create_rc_heap_type` updated for V4 layout
- [x] `test_rc_header_is_32_bytes` test updated for V5 layout (was `test_rc_header_is_24_bytes` in V4)
- [x] All hardcoded "24" references in doc comments updated to 32 (allocate.rs, slice_encoding, hash_table.rs, mod.rs, docs/ including string-sso.md and collections-cow.md)
- [x] `ori_rc_realloc` SAFETY comment updated; `elem_dec_fn` preservation through realloc documented
- [x] All `data_ptr.sub(8)` for strong_count confirmed still correct (spot-check list_rc.rs, map_rc.rs, set_rc.rs, mod.rs, debug.rs, reset/mod.rs, tests.rs)
- [x] `IsShared` GEP -8 offset in `arc_emitter/instr_dispatch.rs:329` verified correct in V4
- [x] Semantic pin test: buffer with stored `elem_dec_fn`, dec with NULL parameter, cleanup happens via header
- [x] `reset/mod.rs` `is_unique()` replaced with `ori_rc_is_unique()` call (data race fix)
- [x] `map_buffer_cleanup` `_len` parameter removed
- [x] `ori_rc_realloc` trace uses `rc_trace_realloc()` helper (not raw `eprintln!`)
- [x] All decorative banners removed from `mod.rs`, `debug.rs`, `hash_table.rs`
- [x] All RC runtime tests pass (`timeout 150 cargo test -p ori_rt`) — 343 passed
- [x] No regressions in AOT tests (`timeout 150 cargo test -p ori_llvm --test aot`) — 1729 passed
- [ ] Valgrind clean on existing heap-allocating tests — 59/62 pass as of 2026-03-20; 3 COW failures remain:
  - [ ] `tests/valgrind/cow/cow_leak_scenarios.ori` — 3 invalid reads (COW element access after free)
  - [ ] `tests/valgrind/cow/cow_map_insert_remove.ori` — 4 invalid reads (COW map element access)
  - [ ] `tests/valgrind/cow/cow_nested.ori` — 3 invalid reads (nested COW element access)
  - [x] `tests/valgrind/cow/cow_sharing.ori` — 0 errors, 0 leaks (verified 2026-03-20)
  - [x] `tests/valgrind/trmc/trmc_tree_ops.ori` — 0 errors, 0 leaks (fixed 2026-03-20: AIMS backward analysis now propagates Project source demand through cross-block lifetimes)
