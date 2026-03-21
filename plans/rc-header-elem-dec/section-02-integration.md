---
section: "02"
title: "Codegen & Runtime Integration"
status: in-progress
goal: "Wire up LLVM codegen and runtime so elem_dec_fn and elem_count are stored in the RC header at collection construction time, and all buffer-freeing paths read from the header"
depends_on: ["01"]
reviewed: false
third_party_review:
  status: resolved
  updated: 2026-03-20
sections:
  - id: "02.1"
    title: "Store elem_dec_fn and elem_count at Collection Construction"
    status: in-progress
  - id: "02.2"
    title: "Iterator Creation and Drop"
    status: not-started
  - id: "02.3"
    title: "Map and Set Integration"
    status: not-started
  - id: "02.R"
    title: "Third Party Review Findings"
    status: complete
  - id: "02.N"
    title: "Completion Checklist"
    status: in-progress
---

# Section 02: Codegen & Runtime Integration

**Status:** In Progress
**Goal:** Wire up LLVM codegen to store `elem_dec_fn` and `elem_count` in the RC header at collection construction time. Ensure all buffer-freeing paths (COW slow paths, collect, slice materialization) propagate both fields to newly allocated buffers.

**Depends on:** Section 01 (RC header must be extended first).

**Blocks:** Section 01.N has 3 Valgrind failures blocked on this section (map `{int: [int]}`, `{str: int}`, `{str: [int]}` double-frees in `cow_leak_scenarios.ori`, `cow_map_insert_remove.ori`, `cow_nested.ori`). These are map-value/map-key double-frees where standalone RcDec AND `map_buffer_cleanup` both fire. The root cause is that map element cleanup functions are not yet stored/propagated correctly — resolving Section 02.3 (map double-free investigation) should address these. After resolution, re-run the 3 blocked Section 01.N Valgrind tests to confirm.

---

## 02.1 Store elem_dec_fn and elem_count at Collection Construction

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/construction.rs`, `compiler/ori_rt/src/rc/mod.rs`

When a list literal `[a, b, c]` is constructed, the codegen's `emit_construct` (via `CtorKind::ListLiteral`, line 83 of `construction.rs`) allocates a buffer via `ori_list_alloc_data`. After storing elements, it must also store `elem_dec_fn` and `elem_count` in the buffer's RC header.

### Runtime FFI Functions

- [x] Extract element header helpers (`store_elem_dec_fn`, `load_elem_dec_fn`, `store_elem_dec_fn_once`, `store_elem_count`, `load_elem_count`, lines 108-213, ~105 lines) from `compiler/ori_rt/src/rc/mod.rs` into a new `rc/elem_header.rs` submodule. `rc/mod.rs` reduced from 501 → 384 lines. (2026-03-20)
- [x] Add runtime function `ori_buffer_store_elem_dec(data: *mut u8, elem_dec_fn: Option<extern "C" fn(*mut u8)>)` in `rc/elem_header.rs` -- wrapper around `store_elem_dec_fn`, `#[no_mangle] extern "C"` callable from LLVM IR (2026-03-20)
- [x] Add runtime function `ori_buffer_store_elem_count(data: *mut u8, count: i64)` in `rc/elem_header.rs` -- wrapper around `store_elem_count`, `#[no_mangle] extern "C"` callable from LLVM IR (2026-03-20)
- [x] Add `load_elem_dec_fn_const(data: *const u8) -> Option<...>` overload (and `load_elem_count_const`) in `rc/elem_header.rs` (2026-03-20)

### LLVM IR Declarations

- [x] Declare `ori_buffer_store_elem_dec` in `compiler/ori_llvm/src/codegen/runtime_decl/runtime_functions.rs` -- signature: `(ptr, ptr) -> void` (2026-03-20)
- [x] Declare `ori_buffer_store_elem_count` in `compiler/ori_llvm/src/codegen/runtime_decl/runtime_functions.rs` -- signature: `(ptr, i64) -> void` (2026-03-20)

### List Construction Codegen

- [x] In the `Construct` handler for lists (`construction.rs`, `CtorKind::ListLiteral` arm): emit `ori_buffer_store_elem_dec(data_ptr, elem_dec_fn)` after storing elements (2026-03-20)
- [x] In the same arm: emit `ori_buffer_store_elem_count(data_ptr, count)` (2026-03-20)
- [x] For scalar elements: `elem_dec_fn` is null — call is idempotent (writes null over zero-init). `elem_count` always stored. (2026-03-20)
- [x] For `ori_list_alloc_data`: no change needed — `ori_rc_alloc` zero-initializes the header. (2026-03-20)

### Set Construction Codegen

- [x] Set construction via `CtorKind::SetLiteral`: emit both `ori_buffer_store_elem_dec` and `ori_buffer_store_elem_count` after element insertion (2026-03-20)

### Collection Reuse Codegen

- [x] `emit_collection_reuse()`: emit both header-store calls after `ori_list_reset_buffer` returns the new buffer (2026-03-20)

### COW Fast-Path Reallocation

- [x] **Verify `ori_rc_realloc` preserves all 32 header bytes**: COW mutations on the fast path (unique owner, needs more capacity) call `ori_rc_realloc`, which preserves all 32 header bytes because `realloc` preserves `min(old, new)` bytes and the header is at the front. Verified via `test_rc_realloc_preserves_header_fields` test at `compiler/ori_llvm/src/tests/runtime_tests.rs:327` — stores elem_dec_fn + elem_count, reallocs, confirms both preserved. (2026-03-20)

### COW Slow Path Propagation

ALL runtime functions that allocate new list buffers via `ori_rc_alloc` must propagate `elem_dec_fn` and `elem_count` from old to new buffer. Two propagation strategies:

- **Direct copy** (preferred when old buffer is available): read `elem_dec_fn`/`elem_count` from old header, write to new header (~3 lines per function).
- **Deferred store** (fallback when no old buffer exists, e.g., `ori_iter_collect`): rely on the next `ori_buffer_rc_dec` call to store via `store_elem_dec_fn_once`, and store `elem_count` explicitly after the allocation completes.

For COW slow paths, `elem_count` on the new buffer = number of elements actually copied (may differ from old `elem_count` for `pop_cow`/`remove_cow` which reduce element count by 1).

- [x] `cow.rs`: `ori_list_push_cow` (slow path), `ori_list_pop_cow` (slow path), `ori_list_set_cow` (slow path) -- `propagate_elem_header()` helper + calls at all 3 sites (2026-03-20)
- [x] `cow_structural.rs`: `ori_list_insert_cow` (slow path), `ori_list_remove_cow` (slow path) -- direct copy via `store_elem_dec_fn`/`store_elem_count` at both sites (2026-03-20)
- [x] `cow_sort.rs`: 4 allocation sites — `propagate_header()` helper + calls at `concat_cow` (2 sites), `reverse_cow`, `sort_cow`. File at 499 lines (under limit via condensed helper + tightened module docs) (2026-03-20)
- [x] `query.rs`: `ori_list_reverse` and `ori_list_concat` -- uses `load_elem_dec_fn_const` for `*const u8` source data (2026-03-20)
- [x] `slice.rs`: `ori_list_materialize_slice` -- reads from ORIGINAL buffer via `load_elem_dec_fn(original)`, not slice data pointer (2026-03-20)
- [x] `mod.rs`: `ori_list_ensure_capacity` (line 61, new alloc at line 85, realloc at line 88) -- the `ori_rc_realloc` grow path preserves the header automatically. The `ori_rc_alloc` first-allocation path (empty sentinel to first buffer) creates a buffer with zero-initialized `elem_dec_fn` and `elem_count`. **JIT/test-only**: declared in `runtime_functions.rs` (line 1218) with JIT mapping (line 175), but NOT referenced from any `arc_emitter/` codegen code. For JIT paths, the header is populated by the first `ori_buffer_rc_dec` call via `store_elem_dec_fn_once`. No codegen changes needed. Stale "8-byte refcount header" comment at line 83 tracked in Cleanup section. (2026-03-20, analysis complete)
- [x] `mod.rs`: `ori_list_new` (line 153, `ori_rc_alloc` at line 162) -- allocates a data buffer for a heap-allocated `OriList`. **JIT/test-only**: zero references from `arc_emitter/` codegen code. Header populated by first `ori_buffer_rc_dec` via `store_elem_dec_fn_once`. No codegen changes needed. Stale "Used by AOT code" doc comment tracked in Cleanup section. (2026-03-20, analysis complete)
- [x] `mod.rs`: `ori_list_push_new` (line 304) -- allocates a new buffer via `ori_rc_alloc` (line 319) with no old buffer to copy from. **JIT/test-only**: zero references from `codegen/arc_emitter/`. Header populated by first `ori_buffer_rc_dec` via `store_elem_dec_fn_once`. No codegen changes needed. Dead declaration cleanup tracked in Section 03.2.5. (2026-03-20, analysis complete)
- [x] `mod.rs`: `ori_list_push` (line 228) -- **IS called from codegen** (references at `apply.rs:208` and `terminators.rs:411`). The first-growth path (line 244-245) calls `ori_rc_alloc` when the list starts empty, creating a buffer with zero-initialized header. Since `ori_list_push` mutates an existing `OriList` via pointer (it doesn't return a new buffer), the codegen cannot emit header-store calls after the push. `elem_dec_fn` populated by first `ori_buffer_rc_dec` via `store_elem_dec_fn_once`. `elem_count` NOT set on first-alloc path. **Risk constrained**: both codegen call sites are for-yield loops (`apply.rs:208`, `terminators.rs:411`) — buffer always created by `ori_list_new`, result always goes through `ori_buffer_rc_dec` before any slice. Slice before RcDec is not reachable in the for-yield pattern. (2026-03-20, verification complete)
- [x] `mod.rs`: `write_array_to_list` -- added `elem_dec_fn` parameter, stores both `elem_dec_fn` and `elem_count` in header after copy. All callers updated: `ori_str_chars` passes `None` (scalar `[char]`), `ori_set_to_list` empty path passes `None`, `ori_map_keys_to_list`/`ori_map_values_to_list` empty paths pass `None`. (2026-03-20)
- [x] `reset/mod.rs`: `ori_list_reset_buffer` (line 34) -- creates new buffer when reuse fails. Does NOT need internal propagation; codegen handles it externally via `ori_buffer_store_elem_dec` + `ori_buffer_store_elem_count` calls after the reset returns (verified: `construction.rs:483-490` emits both header-store calls after reset). (2026-03-20, verification complete)
- [x] `iterator/consumers.rs`: `ori_iter_collect` -- stores `elem_count(data, len)` after collection loop completes. `elem_dec_fn` stored by codegen after collect returns (LLVM-generated thunk). (2026-03-20)
- [ ] `iterator/consumers.rs`: `ori_iter_collect_set` (line 85) -- no old buffer exists; use deferred store. Sets currently read `elem_dec_fn` from the parameter (not header), but storing in the header provides defense-in-depth. **Codegen fix required**: after `emit_iter_collect_set` (in `builtins/iterator_consumers.rs` line 51), emit `ori_buffer_store_elem_dec(result_data, elem_dec_fn)` on the output set buffer. Extract `data` from the loaded set struct via `extract_value`. This mirrors the pattern needed for `emit_iter_collect` (list collect).

### `ori_args_from_argv` — Buffer-Creating Function

`ori_args_from_argv` (line 303 of `lib.rs`) allocates a `[OriStr]` list buffer via `ori_rc_alloc` (line 315). This creates a `[str]` list that contains heap-allocated strings requiring `elem_dec_fn` for cleanup. The function is called from `generate_main_wrapper` in `entry_point.rs` for `@main(args: [str])` signatures.

- [x] `ori_args_from_argv` stores `elem_count` in the RC header after populating elements. Option (b) chosen: `elem_dec_fn` deferred to first `ori_buffer_rc_dec` via `store_elem_dec_fn_once` — safe because `args` is a local binding that always gets RcDec on scope exit, cannot be sliced before then. No ABI change needed. (2026-03-20)

### Set COW Slow Path Propagation

Set COW functions allocate new hash table buffers via `alloc_set_hash_buffer` (which calls `ori_rc_alloc` internally) or `rehash_set`. Each new buffer must have `elem_dec_fn` stored in its header for defense-in-depth with `ori_set_buffer_rc_dec`.

**Note on `elem_count` for sets**: Sets use metadata scanning (not `elem_count`) for element cleanup — `ori_set_buffer_rc_dec` iterates `META_OCCUPIED` buckets, not a contiguous array. The `elem_count` header field is only meaningful for list-style contiguous buffers (used by `slice_buffer_rc_dec`). Sets cannot be sliced. Therefore, `elem_count` does NOT need to be stored for set hash table buffers — only `elem_dec_fn` is relevant.

- [x] `set/cow/basic.rs`: `ori_set_insert_cow` — reads `elem_dec_fn` from old buffer header, passes to `rehash_set` (fast path) and `alloc_set_hash_buffer` (slow path). (2026-03-20)
- [x] `set/cow/basic.rs`: `ori_set_remove_cow` — reads `elem_dec_fn` from old buffer header, passes to `alloc_set_hash_buffer`. (2026-03-20)
- [x] `set/cow/algebra.rs`: `ori_set_union_cow` — `rehash_and_merge_set2` reads `elem_dec_fn` from `d1` header and passes to `rehash_set`. `set1 empty` path reads from `d2` header. (2026-03-20)
- [x] `set/cow/algebra.rs`: `ori_set_intersection_cow` — reads `elem_dec_fn` from `d1` header, passes to `alloc_set_hash_buffer`. (2026-03-20)
- [x] `set/cow/algebra.rs`: `ori_set_difference_cow` — reads `elem_dec_fn` from `d1` header, passes to `alloc_set_hash_buffer`. (2026-03-20)
- [ ] `set/mod.rs`: `ori_set_to_list` (line 55, `ori_rc_alloc` at line 75) -- creates a LIST buffer from set contents. This is a list buffer, so BOTH `elem_dec_fn` AND `elem_count` must be stored. `elem_count` = number of elements copied. `elem_dec_fn` must be passed by caller or deferred.
- [x] `set/mod.rs`: `alloc_set_hash_buffer` — centralized: added `elem_dec_fn` parameter, stores in header internally. All callers pass `elem_dec_fn` (from old header or `None` for fresh allocs). Covers `ori_set_literal_alloc`, `ori_iter_collect_set`, all set COW functions. (2026-03-20)
- [x] `map/hash_table.rs`: `rehash_set` — centralized: added `elem_dec_fn` parameter, stores in header internally. All callers pass `elem_dec_fn` (from old header or `None`). Covers `ori_set_insert_cow`, `ori_set_union_cow`, `ori_iter_collect_set`. (2026-03-20)

### Map COW Slow Path Propagation

Map COW functions allocate new hash table buffers. Maps use the codegen-based approach (option c) for key/value dec functions, not the header. However, `rehash_map` and `OriMap::alloc_hash_buffer` in `map/hash_table.rs` call `ori_rc_alloc`. Since maps do NOT use the `elem_dec_fn` header slot (they need TWO functions), no header propagation is needed for map hash table buffers. `map/cow.rs:144` (`ori_rc_alloc` in map COW slow path) is also a map hash table buffer — explicitly excluded, no action needed.

But map functions that create LIST buffers DO need propagation:

- [ ] `map/mod.rs`: `ori_map_keys_to_list` (line 97, `ori_rc_alloc` at line 118) -- creates a `[K]` list. Needs `elem_dec_fn` + `elem_count` on the output list buffer. See `write_array_to_list_from_data` design decision below.
- [ ] `map/mod.rs`: `ori_map_values_to_list` (line 141, `ori_rc_alloc` at line 164) -- creates a `[V]` list. Same requirement.

### Buffer-Creating Runtime Functions That Need Header Stores

Several runtime functions allocate new list buffers but lack access to `elem_dec_fn`. Each needs a different approach:

**`write_array_to_list`** (line 394 of `list/mod.rs`) — only used by `ori_str_chars` (which produces `[char]`/`[i32]`, scalar elements, NULL `elem_dec_fn`). Since the only real caller uses scalar elements, adding `elem_dec_fn` is not strictly needed. However, for correctness and future safety, extend the signature:
- [x] Add `elem_dec_fn` parameter to `write_array_to_list` function signature (2026-03-20)
- [x] Store `elem_dec_fn` and `elem_count(new_data, n)` inside `write_array_to_list` after the copy (2026-03-20)
- [x] Update `ori_str_chars` to pass `None` (scalar elements) (2026-03-20)

**`write_array_to_list_from_data`** (line 313 of `map/mod.rs`) — used by `ori_map_keys_to_list` and `ori_map_values_to_list`. This takes ownership of an already-allocated buffer but does NOT store `elem_dec_fn`/`elem_count` in it. The buffer was allocated by the caller (not by `write_array_to_list`), so header stores must happen at the allocation site:
- [x] In `ori_map_keys_to_list`: added `key_dec_fn` parameter, stores both `elem_dec_fn` and `elem_count` in RC header after `ori_rc_alloc`. (2026-03-20)
- [x] In `ori_map_values_to_list`: added `val_dec_fn` parameter, stores both `elem_dec_fn` and `elem_count` in RC header after `ori_rc_alloc`. (2026-03-20)
- [x] LLVM IR declarations updated: `ori_map_keys_to_list` adds `Ty::Ptr` for `key_dec_fn`, `ori_map_values_to_list` adds `Ty::Ptr` for `val_dec_fn`. (2026-03-20)
- [x] Codegen call sites updated in `map_builtins.rs`: `emit_map_keys` passes `self.get_or_generate_elem_dec_fn(key_ty)`, `emit_map_values` passes `self.get_or_generate_elem_dec_fn(val_ty)`. (2026-03-20)
- [x] **ABI sync point**: all 4 changes (runtime + LLVM decl + codegen) committed together. (2026-03-20)

**`ori_str_split`** (line 45 of `string/ops.rs`) — allocates its own buffer directly via `ori_rc_alloc` at line 107 (NOT through `write_array_to_list`). The result is `[str]` (24-byte `OriStr` elements) which need `elem_dec_fn` for cleanup.

**NOTE**: The element dec function for `str` is an LLVM-generated thunk (created by `get_or_generate_elem_dec_fn` in `element_fn_gen.rs`), NOT a named Rust runtime function. There is no `ori_str_rc_dec` symbol in `ori_rt`. Approach (b) (internal store) is therefore **not feasible** -- the function pointer only exists in LLVM IR space. Use approach (a).

- [x] Added `elem_dec_fn` parameter to `ori_str_split` function signature. (2026-03-20)
- [x] Stores `elem_dec_fn` and `elem_count` in RC header after element population. (2026-03-20)
- [x] LLVM IR declaration updated: adds `Ty::Ptr` for `elem_dec_fn`. (2026-03-20)
- [x] Codegen call site updated: `emit_str_split` now accepts `str_ty: Idx`, passes `self.get_or_generate_elem_dec_fn(str_ty)`. Call site passes `Idx::STR`. (2026-03-20)
- [x] **ABI sync point**: all 4 changes committed together. (2026-03-20)

**`ori_set_to_list`** (line 55 of `set/mod.rs`) — allocates a list buffer directly via `ori_rc_alloc` at line 75. The function itself doesn't call `write_array_to_list` for the main path (only for the empty case at line 66).
- [x] Added `elem_dec_fn` parameter to `ori_set_to_list`, stores both `elem_dec_fn` and `elem_count` in RC header. (2026-03-20)
- [x] LLVM IR declaration updated: adds `Ty::Ptr` for `elem_dec_fn`. (2026-03-20)
- [x] Codegen call site updated: `emit_set_to_list` passes `self.get_or_generate_elem_dec_fn(elem_ty)`. (2026-03-20)
- [x] **ABI sync point**: all 4 changes committed together. (2026-03-20)

### `ori_iter_collect` Design Decision

`ori_iter_collect` creates a new list buffer via `ori_rc_alloc` but has no access to `elem_dec_fn` (it receives only `iter`, `elem_size`, `out_ptr`). The plan says "use deferred store" but this has a gap:

**Risk**: If the collected buffer is sliced before any `ori_buffer_rc_dec` call fires, `slice_buffer_rc_dec` will find `elem_count == 0` (zero-initialized) and skip cleanup. The `elem_dec_fn` header slot would also be NULL.

**Mitigation**: `ori_iter_collect` is always followed by codegen-emitted `ori_buffer_rc_dec` calls when the list goes out of scope. The buffer cannot be sliced before being returned to the codegen level, at which point the codegen can emit `ori_buffer_store_elem_dec` + `ori_buffer_store_elem_count` immediately after the collect call returns.

- [x] **Codegen fix (list collect)**: `emit_iter_collect` now emits `ori_buffer_store_elem_dec(result_data, elem_dec_fn)` and `ori_buffer_store_elem_count(result_data, result_len)` after the runtime collect call. Extracts `data` via `extract_value(result, 2)`, `len` via `extract_value(result, 0)`. (2026-03-20)
- [x] **Codegen fix (set collect)**: `emit_iter_collect_set` now emits `ori_buffer_store_elem_dec(result_data, elem_dec_fn)` inside the function (covers both `iterator_consumers.rs` and `apply_protocols.rs` call paths). `elem_count` not needed for sets. (2026-03-20)

### `ori_list_push_new` Design Decision

`ori_list_push_new` (line 304 of `list/mod.rs`) allocates a new buffer via `ori_rc_alloc` but has no old buffer header to copy from (the original list is borrowed, not consumed) and no `elem_dec_fn` parameter.

**RESOLVED**: `ori_list_push_new` is declared in `runtime_functions.rs` (line 248) and has a JIT symbol mapping in `runtime_mappings.rs` (line 102), but is **NOT referenced from any arc_emitter codegen code** (grep for `"ori_list_push_new"` in `codegen/arc_emitter/` returns zero results). It is JIT/test-only. The header will be populated by the first `ori_buffer_rc_dec` call via `store_elem_dec_fn_once`. No codegen changes needed.

- [x] Determine whether `ori_list_push_new` is called from LLVM codegen or only from test/JIT paths — **JIT/test only** (not called from `arc_emitter/`) (2026-03-20, plan review)
- [ ] **[WASTE]** `ori_list_push_new` is declared in `runtime_functions.rs` but has no codegen callers — remove the declaration from `runtime_functions.rs` and the JIT symbol mapping from `runtime_mappings.rs`. Tracked in Section 03.2.5 for cleanup alongside `ori_iter_from_list` parameter removal.

### Invariant Assertion

- [ ] Add `debug_assert!` in `ori_buffer_rc_dec` (`list_rc.rs` line 72): after `store_elem_dec_fn_once` and before the RC decrement, assert `elem_dec_fn.is_none() || load_elem_dec_fn(data).is_some()`. This catches the case where a caller passes a real `elem_dec_fn` but the header was never populated (indicating a codegen path missed the `ori_buffer_store_elem_dec` call). Placement: must be in `ori_buffer_rc_dec`, NOT in `drop_elements_and_free` -- the latter does not receive the caller's `elem_dec_fn` parameter.

### SSO String Correctness

- [ ] Verify that `generate_elem_dec_fn_body` for `str` elements correctly handles mixed SSO/heap strings: it loads the `OriStr` struct (24 bytes), emits an SSO check via `emit_sso_check`, and only calls `ori_rc_dec` on heap-allocated strings. SSO strings (<= 23 bytes inline) are skipped. This behavior must match `dec_value_rc_inner` (`rc_value_traversal.rs` line ~183, `Tag::Str` arm).

### AOT Tests

All tests use `ORI_CHECK_LEAKS=1` to verify zero leaks unless otherwise noted.

- [ ] `[str]` list goes out of scope without iteration -- verify zero leaks
- [ ] `[[int]]` nested list goes out of scope -- verify zero leaks
- [ ] `[str]` COW push on shared list (push creates new buffer) -- verify new buffer has correct `elem_dec_fn` and zero leaks
- [ ] `[str]` with mixed SSO and heap strings -- list with `"hi"` (SSO, 2 bytes) and `"a_string_longer_than_twenty_three_bytes"` (heap) -- verify zero leaks
- [ ] `ori_iter_collect` on `[str]` via `for w in words yield w` -- verify collected buffer has correct `elem_dec_fn` in header and zero leaks
- [ ] `map.keys()` on `{str: int}` -- exercises `ori_map_keys_to_list` creating a new `[str]` buffer via direct `ori_rc_alloc` + `write_array_to_list_from_data`; verify zero leaks
- [ ] `str.split(sep:)` returning `[str]` -- exercises `ori_str_split` creating a new `[OriStr]` buffer via direct `ori_rc_alloc`; verify zero leaks

### Cleanup

- [ ] **[WASTE]** `construction.rs` lines 89, 136, 189, 424 -- fallback `_ => ori_types::Idx::INT` silently returns INT as element type on TypeInfo mismatch. Add `tracing::warn!` or `debug_assert!` at each site so misclassification is visible. Affected arms: `ListLiteral` (89), `MapLiteral` (136), `SetLiteral` (189), `emit_collection_reuse` (424).
- [ ] **[DRIFT]** `list_rc.rs:27` -- Doc comment on `drop_elements_and_free` says "V4: at `header_data - 16`". Update to "V5: at `header_data - 24`" (`ELEM_DEC_FN_OFFSET` is now 24).
- [ ] **[DRIFT]** `list/mod.rs:83` -- Comment says "8-byte refcount header". Update to "32-byte RC header (V5)".
- [ ] **[DRIFT]** `list/mod.rs:103` -- Comment says "RC header at `ptr - 8`". Update to "RC header at `ptr - 32`; strong_count at `ptr - 8`".
- [ ] **[DRIFT]** `list/mod.rs:131` -- Comment says "RC-managed (8-byte refcount header, initial count = 1)". Update to "32-byte RC header (V5), initial count = 1".
- [ ] **[DRIFT]** `list/mod.rs:199` -- Comment says "RC-managed with 8-byte header" in `ori_list_free_data` doc. The "8-byte" refers to the alignment parameter passed to `ori_rc_free`, not the header size -- but the comment is misleading. Reword to "32-byte RC header (V5), alignment 8" for clarity.
- [ ] **[DRIFT]** `cow.rs:38` -- Doc comment on `ori_list_push_cow` slow path references "§02.7" (stale plan numbering). Update to reference the current header-based cleanup model.
- [ ] **[DRIFT]** `list/reset/mod.rs:71` -- Comment says "V4: `strong_count` at `data_ptr - 8`". Update version label to V5 (offset itself is correct).
- [ ] **[DRIFT]** `list/mod.rs:151` -- Doc comment on `ori_list_new` says "Used by AOT code." but grep confirms zero codegen callers — update to "Used by JIT/test code."
- [ ] **[STYLE]** `list/mod.rs` lines 96, 294 -- 2 decorative banners. Replace with plain section comments.
- [ ] **[STYLE]** `iterator/consumers.rs` lines 11, 76, 159, 181, 214, 247, 303, 330 -- 8 decorative banners. Replace with plain section comments.
- [ ] **[WASTE]** `cow_sort.rs:256` -- `ori_list_reverse_cow` fast path allocates `vec![0u8; es]` on every call. For common element sizes (8, 16, 24 bytes), use a stack array `[0u8; 24]` with heap fallback for larger sizes.
- [ ] **[WASTE]** `cow_sort.rs:458` -- `apply_permutation_in_place` allocates `vec![0u8; elem_size]` similarly. Same fix: stack array `[0u8; 24]` with heap fallback. Address alongside line 256 since both are in the same file.
- [ ] **[LATENT]** `query.rs` lines 142, 199 -- `ori_list_reverse` and `ori_list_concat` use hardcoded alignment `8` in `ori_rc_alloc`. Safe for current Ori types (max align 8) but incorrect if element alignment exceeds 8. Add `elem_align` parameter or use a centralized alignment lookup.
- [ ] **[WASTE]** `map/mod.rs:21-25` -- `#[allow(unused_imports, reason = "used by cow.rs after rewrite")]` masks actual unused import: `META_EMPTY` is NOT used by `cow.rs` (only `META_OCCUPIED` and `META_TOMBSTONE` are). Either remove `META_EMPTY` from the re-export or change to `#[expect(unused_imports)]` so the lint fires when the situation changes.
- [ ] **[WASTE]** `map/mod.rs:329` -- `let _ = elem_size;` in `write_array_to_list_from_data` explicitly discards the `elem_size` parameter. The function accepts it but never uses it (callers already sized the buffer externally). Either remove the parameter (and update 2 callers at lines 133, 179) or use it for a `debug_assert!` on buffer size.
- [ ] **[WASTE]** `set/cow/basic.rs` lines 38, 147 and `set/cow/algebra.rs` lines 80, 192, 312 -- `let _ea = elem_align.max(1) as usize;` computes alignment then discards it with underscore prefix. Five occurrences across two files. The `elem_align` parameter is accepted from codegen but never used (all `alloc_set_hash_buffer` and `rehash_set` calls hardcode alignment `8`). Either use `_ea` in the allocation calls or remove the dead computation.
- [ ] **[STYLE]** `set/mod.rs:98` -- decorative banner `// ── Literal Construction ──`. Replace with plain section comment.
- [ ] **[STYLE]** `map/mod.rs:228` -- decorative banner `// ── Literal Construction ──`. Replace with plain section comment.
- [ ] **[BLOAT]** `construction.rs` is 499 lines -- at the limit. If any further work is needed in this file, extract `emit_variant_via_alloca` and `emit_variant_via_insertvalue` (total ~130 lines) into a `variant_construction.rs` submodule before exceeding 500 lines.

---

## 02.2 Iterator Creation and Drop

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/builtins/collections/list_builtins.rs`, `compiler/ori_rt/src/iterator/state.rs`, `compiler/ori_rt/src/iterator/sources.rs`

**STATUS UPDATE**: The `iter-rc-contract` plan (2026-03-18) already fixed `emit_list_iter` (fn at line 126 of `list_builtins.rs`, fix at line 144) to pass `self.get_or_generate_elem_dec_fn(elem_ty)` instead of NULL. `IterState::List` Drop (line 170 of `state.rs`) passes this stored `elem_dec_fn` to `ori_buffer_rc_dec`. The iterator already carries the real function. This section's remaining work is verifying the header provides a second safety net and deciding on parameter cleanup (Section 03).

With `elem_dec_fn` ALSO stored in the header at construction time (Section 02.1), there are now TWO mechanisms for element cleanup: (1) the parameter passed by the caller to `ori_buffer_rc_dec`, and (2) the header-stored function read by `ori_buffer_rc_dec`. The `store_elem_dec_fn_once` write-once pattern ensures they agree. This provides defense-in-depth: even if one path passes NULL, the header provides the function.

### Parameter Retention Decision

- [ ] **Decide**: Keep or remove the `elem_dec_fn` parameter in `ori_iter_from_list`?
  - **Option A (recommended for this section)**: Keep parameter for defense-in-depth. Both the parameter and header provide `elem_dec_fn`. Section 03 removes the parameter once the header-based approach is proven stable by the Section 04 test matrix.
  - **Option B**: Remove parameter now. Runtime reads from header only. Correct long-term state but removes a safety net before the header approach is battle-tested.
  - Record the decision in this section's completion notes.

### Integration Verification

- [ ] Verify: when iterator's `ori_buffer_rc_dec` call reaches zero, it reads `elem_dec_fn` from the header and performs element cleanup
- [ ] Verify: when explicit RcDec's `ori_buffer_rc_dec` call reaches zero, same behavior -- reads from header
- [ ] Verify: `store_elem_dec_fn_once` CAS handles the case where both iterator Drop and explicit RcDec store to the same header -- first non-NULL wins, second is a no-op. Section 01 has a unit test (`elem_dec_fn_store_once_first_non_null_wins`), but verify the end-to-end runtime path exercises it under the two-dec scenario (iterator + explicit RcDec on the same buffer).

### AOT Tests

- [ ] `[str]` iteration where iterator dec reaches zero first -- elements cleaned via header function, zero leaks
- [ ] `[str]` iteration where explicit dec reaches zero first -- same behavior, zero leaks
- [ ] Function parameter `[str]` -- callee iterates, caller uses after -- no double-free, no leak
- [ ] `[str]` iteration + slice -- create list, take slice, iterate original via `for w in list do body` -- verify both iterator and slice store/read from the SAME buffer's header. When either is the last owner, elements are cleaned up correctly. Zero leaks, no double-free.

### Cleanup

- [ ] **[NOTE]** `list_builtins.rs:115-125` -- Doc comment on `emit_list_iter` documents that real `elem_dec_fn` is passed. After header-based approach is added, update doc to mention the header as a second safety net.
- [ ] **[DRIFT]** `iterator/state.rs:49` -- Comment says `cap != 0` indicates RC-managed data but does not mention V5 header layout. `IterState::List` Drop at line 170 calls `ori_buffer_rc_dec` which reads `elem_dec_fn` from the header. Update the doc to note that cleanup relies on the header's `elem_dec_fn` (not just the `elem_dec_fn` field in the struct).

---

## 02.3 Map and Set Integration

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/rc_buffer_ops.rs`, `compiler/ori_rt/src/map/mod.rs`, `compiler/ori_rt/src/set/cow/basic.rs`, `compiler/ori_rt/src/set/cow/algebra.rs`, `compiler/ori_rt/src/set/mod.rs`

**STATUS UPDATE**: The `iter-rc-contract` plan (2026-03-18) already fixed `emit_map_iter` to pass real `key_dec_fn`/`val_dec_fn` instead of NULL. Maps use the codegen-based approach (option c from Section 01.3). The `__for_coll_N` phantom only matches `List | Set` (not `Map`), but maps use ownership transfer (`@iter(%map [own])`) where the iterator's Drop is the sole cleanup path.

### Maps (Codegen-Based, Already Implemented)

- [ ] Verify `emit_map_iter` (line 331 of `map_builtins.rs`) passes real `key_dec_fn`/`val_dec_fn` to `ori_iter_from_map`. Verify `IterState::Map` Drop at line 204 correctly passes `*key_dec_fn` and `*val_dec_fn` to `ori_map_buffer_rc_dec`. No header changes needed for maps -- maps need TWO dec functions that cannot fit in a single header slot, so the codegen-based approach is correct.

### Map Double-Free Investigation

- [ ] Investigate the 3 blocked Section 01.N Valgrind failures (`cow_leak_scenarios.ori`, `cow_map_insert_remove.ori`, `cow_nested.ori`). Root cause hypothesis: `emit_buffer_rc_dec_map` (`rc_buffer_ops.rs` line 70) emits an RcDec for map values/keys AND `map_buffer_cleanup` also iterates and decs elements -- if both fire, that is a double-free. Determine: (a) does `ori_map_buffer_rc_dec` call element cleanup when RC reaches zero? (b) does the AIMS pipeline also emit per-element RcDec independently? If both fire for the same elements, one must be removed.

### Sets (Header-Based, Same as Lists)

Set construction codegen is complete (see 02.1 "Set Construction Codegen" -- `elem_dec_fn` and `elem_count` stored at literal construction time). Remaining work:

- [ ] Verify set iteration codegen: `emit_list_iter` is also called for sets (dispatched at `builtins/collections/mod.rs` line 463 as `("Set", "iter") => emit_list_iter`). Sets share the `ori_buffer_rc_dec` cleanup path with lists for iterator Drop. Note: sets use `ori_set_buffer_rc_dec` (not `ori_buffer_rc_dec`) for standalone RcDec -- the set variant scans metadata for OCCUPIED buckets. The header `elem_dec_fn` provides defense-in-depth for both paths.
- [ ] Set COW slow path propagation: all set COW functions that allocate new hash table buffers must store `elem_dec_fn` in the new buffer's header. See 02.1 "Set COW Slow Path Propagation" for the full list.
- [ ] `ori_set_to_list` (`set/mod.rs` line 55): creates a LIST buffer from set contents -- must store BOTH `elem_dec_fn` and `elem_count`. Requires the set-to-list caller to provide `elem_dec_fn`.

### AOT Tests

- [ ] `{str: int}` map iteration -- verify zero leaks via `ORI_CHECK_LEAKS=1` (run 10x to confirm stability against intermittent failures)
- [ ] `Set<str>` iteration -- verify zero leaks
- [ ] `{str: int}` map passed to function, iterated inside -- same pattern as `test_str_list_passed_to_two_functions` but for maps
- [ ] `Set<str>` passed to function, iterated inside -- verify both header-based and parameter-based cleanup paths work, zero leaks
- [ ] `Set<str>` COW insert on shared set -- verify new buffer has correct `elem_dec_fn` and zero leaks
- [ ] `Set<str>` union/intersection/difference -- verify new buffer cleanup, zero leaks
- [ ] `map.keys()` on `{str: int}` -- verify the output `[str]` list buffer has correct `elem_dec_fn` and zero leaks (exercises `ori_map_keys_to_list` direct alloc + `write_array_to_list_from_data` path)

### Cleanup

- [ ] **[NOTE]** `map_builtins.rs:320-330` -- Doc comment on `emit_map_iter` correctly documents that real dec functions are passed. No update needed unless header-based approach is later extended to maps.
- [ ] **[DRIFT]** `set/cow/basic.rs` and `set/cow/algebra.rs` -- After adding `elem_dec_fn` propagation, update doc comments to note that the new buffer has `elem_dec_fn` stored in its header.

---

## 02.R Third Party Review Findings

- [x] `[TPR-02-001][medium]` `plans/rc-header-elem-dec/section-02-integration.md:1` — Section 02 currently advertises conflicting progress states across its own metadata and the plan index.
  Evidence: This file's frontmatter already says `status: in-progress` and `02.N` is `in-progress`, but the section body still says `**Status:** Not Started`, and `plans/rc-header-elem-dec/index.md:35` still lists Section 02 as `Not Started`.
  Impact: Readers cannot tell whether Section 02 has merely been replanned or has actually begun, which makes dependency tracking and downstream plan updates unreliable.
  Required plan update: Pick a single state for Section 02 and sync the frontmatter, body, `02.N`, and index entry in one pass.
  Resolved: Fixed on 2026-03-20. Body status synced to "In Progress" matching frontmatter; index.md Section 02 status updated to "In Progress".
- [x] `[TPR-02-002][high]` `compiler/ori_llvm/src/codegen/runtime_decl/runtime_functions.rs:141` — The new header-store runtime helpers are marked AOT-only even though the shared ARC emitter now calls them on the JIT path as well.
  Resolved: Fixed on 2026-03-20. Set `jit_allowed: true` for both `ori_buffer_store_elem_dec` and `ori_buffer_store_elem_count` in `runtime_functions.rs`. Added symbol mappings in `runtime_mappings.rs`. `jit_symbol_mappings_match_jit_allowed` test confirms sync. All 1810 AOT tests pass.
- [x] `[TPR-02-003][low]` `compiler/ori_llvm/src/tests/evaluator_tests.rs:1` — The regression fix for JIT symbol availability is not covered by a functional JIT test that actually compiles a collection literal or reuse path through MCJIT.
  Evidence: `cargo test -p ori_llvm evaluator_tests -- --list` shows only 8 evaluator unit tests, all structural; `compiler/ori_llvm/src/tests/evaluator_tests.rs` covers `LLVMValue`, error types, evaluator construction, and empty-module compilation, but no test exercises list/set literal codegen or `ori_list_reset_buffer` on the JIT path.
  Impact: The exact failure mode fixed by TPR-02-002 can regress without being caught by the existing evaluator suite; the current guard rails only prove declaration/mapping consistency (`jit_symbol_mappings_match_jit_allowed`), not end-to-end execution through MCJIT.
  Required follow-up: Add a focused evaluator integration test that JIT-compiles a function using at least one list or set literal, and ideally a collection-reuse path, then executes it successfully.
  Resolved: Accepted on 2026-03-20. Finding is factually correct — no functional JIT test exercises collection literal construction. Spec tests exercise JIT indirectly but provide no explicit regression guard. Integrated as a task in 02.N Cleanup.

---

## 02.N Completion Checklist

### Runtime & Codegen Wiring

- [x] `ori_buffer_store_elem_dec` runtime function exists and is callable from LLVM IR (2026-03-20)
- [x] `ori_buffer_store_elem_count` runtime function exists and is callable from LLVM IR (2026-03-20)
- [x] Both declared in `runtime_functions.rs`: `store_elem_dec` as `(ptr, ptr) -> void`, `store_elem_count` as `(ptr, i64) -> void` (2026-03-20)
- [x] `load_elem_dec_fn_const` / `load_elem_count_const` overloads exist for `*const u8` callers (2026-03-20)
- [x] `rc/mod.rs` split complete: element header helpers extracted to `rc/elem_header.rs` (mod.rs: 384 lines) (2026-03-20)
- [x] List construction stores `elem_dec_fn` and `elem_count` in RC header after element storage (2026-03-20)
- [x] Set construction stores `elem_dec_fn` and `elem_count` in RC header after buffer population (2026-03-20)
- [x] Collection reuse (`emit_collection_reuse`) stores both `elem_dec_fn` and `elem_count` after `ori_list_reset_buffer` returns (2026-03-20)
- [x] Map iteration passes real `key_dec_fn`/`val_dec_fn` (not NULL) to `ori_iter_from_map` (implemented by iter-rc-contract plan, 2026-03-18)
- [x] `emit_list_iter` passes real `elem_dec_fn` to `ori_iter_from_list` (implemented by iter-rc-contract plan, 2026-03-18)

### COW & Buffer Propagation

- [x] ALL list COW slow path functions propagate both `elem_dec_fn` and `elem_count` from old to new buffer: `push_cow`, `pop_cow`, `set_cow` (cow.rs), `insert_cow`, `remove_cow` (cow_structural.rs), `concat_cow`, `reverse_cow`, `sort_cow`, `sort_stable_cow` (cow_sort.rs) (2026-03-20)
- [x] ALL set COW slow path functions propagate `elem_dec_fn` to new buffer via centralized `alloc_set_hash_buffer` and `rehash_set`: `insert_cow`, `remove_cow` (set/cow/basic.rs), `union_cow`, `intersection_cow`, `difference_cow` (set/cow/algebra.rs), `ori_iter_collect_set` (iterator/consumers.rs) (2026-03-20)
- [x] `query.rs` functions (`ori_list_reverse`, `ori_list_concat`) propagate both `elem_dec_fn` and `elem_count` via direct copy (2026-03-20)
- [x] `write_array_to_list` extended with `elem_dec_fn` parameter and stores both `elem_dec_fn` + `elem_count` internally. `ori_str_chars` passes `None`. (2026-03-20)
- [ ] `ori_map_keys_to_list` stores `elem_dec_fn` + `elem_count` on list buffer after `ori_rc_alloc` (requires `key_dec_fn` parameter, LLVM decl + codegen update)
- [x] `ori_map_values_to_list` stores `elem_dec_fn` + `elem_count` on list buffer (2026-03-20)
- [x] `ori_str_split` stores `elem_dec_fn` + `elem_count` on list buffer via `elem_dec_fn` parameter + internal store (2026-03-20)
- [x] `ori_set_to_list` stores `elem_dec_fn` + `elem_count` on list buffer via `elem_dec_fn` parameter (2026-03-20)
- [x] LLVM IR declarations updated for all 4 signature changes: `ori_map_keys_to_list`, `ori_map_values_to_list`, `ori_set_to_list`, `ori_str_split` (2026-03-20)
- [x] Codegen call sites updated: `map_builtins.rs`, `set_builtins.rs`, `string_builtins.rs` (2026-03-20)
- [x] `ori_iter_collect` output buffer gets `elem_dec_fn` + `elem_count` via codegen-emitted header-store calls (2026-03-20)
- [x] `ori_iter_collect_set` output buffer gets `elem_dec_fn` via codegen-emitted header-store call inside `emit_iter_collect_set` (2026-03-20)
- [x] `ori_args_from_argv` stores `elem_count` in header; `elem_dec_fn` deferred to first `ori_buffer_rc_dec` (2026-03-20)
- [x] `alloc_set_hash_buffer` centralized with `elem_dec_fn` parameter; stores in header internally (2026-03-20)
- [x] `rehash_set` centralized with `elem_dec_fn` parameter; stores in header on new buffer internally (2026-03-20)
- [x] `ori_rc_realloc` preserves both `elem_dec_fn` and `elem_count` -- verified by `test_rc_realloc_preserves_header_fields` at `compiler/ori_llvm/src/tests/runtime_tests.rs:327` (2026-03-20)

### Invariant & Safety

- [ ] `debug_assert!` in `ori_buffer_rc_dec` catches NULL header with non-NULL caller `elem_dec_fn` (placed in `ori_buffer_rc_dec`, NOT in `drop_elements_and_free`)
- [x] `test_rc_header_is_32_bytes` test existence verified -- exists at `compiler/ori_llvm/src/tests/runtime_tests.rs:289` (2026-03-20)
- [ ] Map double-free root cause identified and Section 01.N blocked Valgrind failures resolved (3 tests)

### AOT Tests & Verification

- [ ] `[str]` list scope drop -- zero leaks
- [ ] `[[int]]` nested list scope drop -- zero leaks
- [ ] `[str]` COW push on shared list -- zero leaks
- [ ] SSO/heap mixed `[str]` -- zero leaks
- [ ] `ori_iter_collect` on `[str]` -- output buffer has correct `elem_dec_fn`, zero leaks
- [ ] `map.keys()` on `{str: int}` -- output `[str]` buffer has `elem_dec_fn`, zero leaks (exercises `ori_map_keys_to_list` direct alloc + `write_array_to_list_from_data` path)
- [ ] `str.split(sep:)` returning `[str]` -- exercises `ori_str_split` direct `ori_rc_alloc` path, zero leaks
- [ ] `[str]` iteration where iterator dec reaches zero first -- zero leaks
- [ ] `[str]` iteration where explicit dec reaches zero first -- zero leaks
- [ ] Function parameter `[str]` -- callee iterates, caller uses after -- no double-free, no leak
- [ ] Iterator + slice cross-feature test -- list with elements, take slice, iterate original -- zero leaks
- [ ] `{str: int}` map iteration -- zero leaks (10x stability check)
- [ ] `Set<str>` iteration -- zero leaks
- [ ] `{str: int}` map passed to function, iterated inside -- zero leaks
- [ ] `Set<str>` passed to function, iterated inside -- zero leaks
- [ ] `Set<str>` COW insert on shared set -- zero leaks
- [ ] `Set<str>` union/intersection/difference -- zero leaks
- [ ] `set.to_list()` on `Set<str>` -- exercises `ori_set_to_list` creating a new `[str]` buffer, zero leaks
- [ ] `ori_iter_collect_set` on `Set<str>` via `for x in items yield x` with set target -- output set buffer has correct `elem_dec_fn`, zero leaks
- [ ] `@main(args: [str])` with arguments -- exercises `ori_args_from_argv` creating `[str]` buffer, zero leaks (run AOT binary with args, verify `ORI_CHECK_LEAKS=1` clean)
- [ ] `test_str_list_passed_to_two_functions` passes reliably (not ignored)
- [ ] `test_nested_list_iteration` passes reliably (not ignored)

### Valgrind

- [ ] No valgrind errors on `[str]` and `[[int]]` iteration patterns
- [ ] No valgrind errors on `{str: int}` map iteration patterns
- [ ] No valgrind errors on `Set<str>` iteration patterns
- [ ] No valgrind errors on `Set<str>` COW mutation patterns (insert, remove, union)
- [ ] No valgrind errors on `map.keys()` / `map.values()` with fat-pointer keys/values
- [ ] No valgrind errors on `set.to_list()` with `Set<str>`
- [ ] No valgrind errors on `str.split(sep:)` returning `[str]`
- [ ] No valgrind errors on `ori_iter_collect_set` with `Set<str>` elements

### ABI Sync Points — All Must Be Single-Commit Changes

All runtime function signature changes below require updating THREE locations atomically (same commit):
1. Runtime function signature in `ori_rt` (Rust)
2. LLVM IR declaration in `runtime_functions.rs` (parameter list)
3. Codegen call site(s) in `arc_emitter/builtins/` (argument passing)

| Function | Runtime File | LLVM Decl Line | Codegen File | New Param |
|----------|-------------|----------------|--------------|-----------|
| `ori_map_keys_to_list` | `map/mod.rs:97` | `runtime_functions.rs:512` | `map_builtins.rs:74` | `key_dec_fn: ptr` |
| `ori_map_values_to_list` | `map/mod.rs:141` | `runtime_functions.rs:520` | `map_builtins.rs:112` | `val_dec_fn: ptr` |
| `ori_str_split` | `string/ops.rs:45` | `runtime_functions.rs:778` | `string_builtins.rs:158` | `elem_dec_fn: ptr` |
| `ori_set_to_list` | `set/mod.rs:55` | `runtime_functions.rs:728` | `set_builtins.rs:270` | `elem_dec_fn: ptr` |

Additionally, if `alloc_set_hash_buffer` and `rehash_set` gain `elem_dec_fn` parameters, their callers within `ori_rt` must be updated (these are internal-only, no LLVM IR declaration needed).

If `ori_args_from_argv` option (a) is chosen: `lib.rs:303` + `runtime_functions.rs` + `entry_point.rs` must also sync.

- [ ] All ABI sync points committed atomically (no partial updates)

### Build Verification

- [ ] All existing AOT tests pass (`timeout 150 cargo test -p ori_llvm --test aot`)
- [ ] All tests pass in release build (`cargo b --release && timeout 150 cargo test -p ori_llvm --test aot`)

### Cleanup

- [ ] Stale "V4: at `header_data - 16`" comment in `list_rc.rs:27` updated to V5
- [ ] Stale "8-byte refcount header" comments in `list/mod.rs` updated to "32-byte RC header (V5)" (lines 83, 103, 131, 199)
- [ ] Stale "§02.7" reference in `cow.rs:38` updated to header-based cleanup model
- [ ] Stale "V4" label in `list/reset/mod.rs:71` updated to V5
- [ ] Decorative banners removed from `list/mod.rs` (2 banners) and `iterator/consumers.rs` (8 banners)
- [ ] `construction.rs` fallback `_ => Idx::INT` patterns have `tracing::warn!` or `debug_assert!` (lines 89, 136, 189, 424)
- [ ] `iterator/state.rs` doc comment updated to mention V5 header dependency for `elem_dec_fn` cleanup
- [ ] `list_builtins.rs` doc comment updated to mention header as second safety net
- [ ] `set/cow/basic.rs` and `set/cow/algebra.rs` doc comments updated for `elem_dec_fn` propagation
- [x] `ori_list_push_new` codegen usage determined -- **JIT/test only** (not called from `arc_emitter/`); no codegen changes needed (2026-03-20)
- [ ] **[TPR-02-003]** Add functional JIT evaluator integration test that compiles a list/set literal through MCJIT and executes successfully (regression guard for JIT symbol availability)
- [ ] Decorative banners removed from `set/mod.rs` (1 banner at line 98) and `map/mod.rs` (1 banner at line 228)
- [ ] `map/mod.rs:21-25` `#[allow(unused_imports)]` cleaned up: remove `META_EMPTY` from re-export (unused by `cow.rs`)
- [ ] `map/mod.rs:329` dead `let _ = elem_size;` in `write_array_to_list_from_data` resolved (remove parameter or add assertion)
- [ ] `set/cow/basic.rs` + `set/cow/algebra.rs` dead `_ea` computations removed or used (5 sites)
- [ ] `cow_sort.rs:458` `vec![0u8; elem_size]` in `apply_permutation_in_place` converted to stack array with heap fallback

### Excluded Allocation Sites (No Action Needed)

The following `ori_rc_alloc` call sites do NOT need `elem_dec_fn` propagation and are explicitly excluded:

- **`string/methods/mod.rs`** (lines 301, 388): String COW operations. These allocate string DATA buffers (raw bytes), not list element buffers. `elem_dec_fn` is for element-level cleanup of collections, not for string internals.
- **`string/mod.rs`** (lines 196, 240, 263): `OriStr::from_bytes`, `with_capacity`, `from_raw`. Same — string data, not collection elements.
- **`string/ops.rs:221`**: `ori_str_concat_cow` — allocates a new string data buffer on the slow path. Not a collection element buffer.
- **`map/hash_table.rs`** (lines 232, 274): `rehash_map`, `OriMap::alloc_hash_buffer`. Map hash table buffers. Maps use TWO cleanup functions (key + value) that cannot fit in a single header slot. The codegen-based approach (option c) handles map cleanup. No header propagation needed.
- **`map/cow.rs:144`**: Map COW slow path. Same as above — map hash table buffer, not list/set buffer.
- **`iterator/sources.rs:93`**: `ori_iter_from_str` — allocates a heap copy of string bytes for the string iterator. Not a collection element buffer.
- **`list/mod.rs:108`**: `ori_list_new` — allocates the `OriList` STRUCT on the heap (not the data buffer). The `ori_rc_alloc` here is for the list metadata struct, not for the data buffer. The data buffer allocation at line 162 is covered separately.
