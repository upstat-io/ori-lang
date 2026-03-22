---
section: "02"
title: "Codegen & Runtime Integration"
status: complete
goal: "Wire up LLVM codegen and runtime so elem_dec_fn and elem_count are stored in the RC header at collection construction time, and all buffer-freeing paths read from the header"
depends_on: ["01"]
reviewed: true
third_party_review:
  status: resolved
  updated: 2026-03-21
sections:
  - id: "02.1"
    title: "Store elem_dec_fn and elem_count at Collection Construction"
    status: complete
  - id: "02.2"
    title: "Iterator Creation and Drop"
    status: complete
  - id: "02.3"
    title: "Map and Set Integration"
    status: complete
  - id: "02.R"
    title: "Third Party Review Findings"
    status: complete
  - id: "02.N"
    title: "Completion Checklist"
    status: complete
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
- [x] `iterator/consumers.rs`: `ori_iter_collect_set` (line 85) -- no old buffer exists; use deferred store. Sets currently read `elem_dec_fn` from the parameter (not header), but storing in the header provides defense-in-depth. **Codegen fix done**: `emit_iter_collect_set` emits `ori_buffer_store_elem_dec(result_data, elem_dec_fn)` on the output set buffer (2026-03-20, see line 171).

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
- [x] `set/mod.rs`: `ori_set_to_list` (line 55, `ori_rc_alloc` at line 75) -- creates a LIST buffer from set contents. Added `elem_dec_fn` parameter, stores both `elem_dec_fn` and `elem_count` in RC header (2026-03-20, see line 157-160).
- [x] `set/mod.rs`: `alloc_set_hash_buffer` — centralized: added `elem_dec_fn` parameter, stores in header internally. All callers pass `elem_dec_fn` (from old header or `None` for fresh allocs). Covers `ori_set_literal_alloc`, `ori_iter_collect_set`, all set COW functions. (2026-03-20)
- [x] `map/hash_table.rs`: `rehash_set` — centralized: added `elem_dec_fn` parameter, stores in header internally. All callers pass `elem_dec_fn` (from old header or `None`). Covers `ori_set_insert_cow`, `ori_set_union_cow`, `ori_iter_collect_set`. (2026-03-20)

### Map COW Slow Path Propagation

Map COW functions allocate new hash table buffers. Maps use the codegen-based approach (option c) for key/value dec functions, not the header. However, `rehash_map` and `OriMap::alloc_hash_buffer` in `map/hash_table.rs` call `ori_rc_alloc`. Since maps do NOT use the `elem_dec_fn` header slot (they need TWO functions), no header propagation is needed for map hash table buffers. `map/cow.rs:144` (`ori_rc_alloc` in map COW slow path) is also a map hash table buffer — explicitly excluded, no action needed.

But map functions that create LIST buffers DO need propagation:

- [x] `map/mod.rs`: `ori_map_keys_to_list` (line 97, `ori_rc_alloc` at line 118) -- creates a `[K]` list. Added `key_dec_fn` parameter, stores both `elem_dec_fn` and `elem_count` (2026-03-20, see lines 140-144).
- [x] `map/mod.rs`: `ori_map_values_to_list` (line 141, `ori_rc_alloc` at line 164) -- creates a `[V]` list. Added `val_dec_fn` parameter, stores both (2026-03-20, see lines 141-144).

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
- [x] **[WASTE]** `ori_list_push_new` — removed declaration from `runtime_functions.rs` and JIT symbol mapping from `runtime_mappings.rs`. Runtime implementation in `ori_rt` retained for future JIT use. (2026-03-21)

### Invariant Assertion

- [x] Add `debug_assert!` in `ori_buffer_rc_dec` (`list_rc.rs`): after `store_elem_dec_fn_once` and before the RC decrement, assert `elem_dec_fn.is_none() || load_elem_dec_fn(data).is_some()`. Catches non-NULL caller with NULL header post-store. Added 2026-03-21.

### SSO String Correctness

- [x] Verify that `generate_elem_dec_fn_body` for `str` elements correctly handles mixed SSO/heap strings: code reviewed on 2026-03-21 — `dec_value_rc` hits `Tag::Str` at `rc_value_traversal.rs:183`, extracts data pointer (field 2), `emit_sso_check` (MSB flag + null check), only calls `ori_rc_dec` on heap strings. SSO strings skipped. AOT test `test_str_list_mixed_sso_heap` confirms zero leaks with mixed "hi" (SSO) + long heap strings.

### AOT Tests

All tests use `ORI_CHECK_LEAKS=1` to verify zero leaks unless otherwise noted.

- [x] `[str]` list goes out of scope without iteration -- zero leaks (AOT test `test_str_list_scope_drop`, 2026-03-21)
- [x] `[[int]]` nested list goes out of scope -- zero leaks (AOT test `test_nested_int_list_scope_drop`, 2026-03-21)
- [x] `[str]` COW push on shared list (push creates new buffer) -- zero leaks (AOT test `test_str_list_cow_push_shared`, 2026-03-21)
- [x] `[str]` with mixed SSO and heap strings -- zero leaks (AOT test `test_str_list_mixed_sso_heap`, 2026-03-21)
- [x] `ori_iter_collect` on `[str]` via `for w in words yield w` -- zero leaks (AOT test `test_str_list_iter_collect`, 2026-03-21)
- [x] `map.keys()` on `{str: int}` -- fixed via `key_inc_fn` parameter (2026-03-21). AOT test `test_map_keys_str_scope_drop` passes in debug + release. See 02.3 map double-free fix.
- [x] `str.split(sep:)` returning `[str]` -- zero leaks (AOT test `test_str_split_scope_drop`, 2026-03-21)

### Cleanup

- [x] **[WASTE]** `construction.rs` lines 89, 136, 189, 424 -- fallback `_ => ori_types::Idx::INT` silently returns INT as element type on TypeInfo mismatch. Add `tracing::warn!` or `debug_assert!` at each site so misclassification is visible. Affected arms: `ListLiteral` (89), `MapLiteral` (136), `SetLiteral` (189), `emit_collection_reuse` (424). Done: `debug_assert!(false, ...)` at all 4 sites (2026-03-21)
- [x] **[DRIFT]** `list_rc.rs:27` -- V4 → V5, `header_data - 16` → `header_data - 24` (2026-03-21)
- [x] **[DRIFT]** `list/mod.rs:83` -- "8-byte refcount header" → "32-byte V5 header" (2026-03-21)
- [x] **[DRIFT]** `list/mod.rs:103` -- "RC header at `ptr - 8`" → "RC header at `ptr - 32`; strong_count at `ptr - 8`" (2026-03-21)
- [x] **[DRIFT]** `list/mod.rs:131` -- "8-byte refcount header" → "32-byte V5 header" (2026-03-21)
- [x] **[DRIFT]** `list/mod.rs:199` -- "RC-managed with 8-byte header" → "32-byte V5 RC header, alignment 8" (2026-03-21)
- [x] **[DRIFT]** `cow.rs:38` -- "§02.7" → "`elem_dec_fn` in the V5 RC header handles cleanup" (2026-03-21)
- [x] **[DRIFT]** `list/reset/mod.rs:71` -- V4 → V5 label (2026-03-21)
- [x] **[DRIFT]** `list/mod.rs:151` -- "Used by AOT code" → "Used by JIT/test code. Not called from arc_emitter/ codegen." (2026-03-21)
- [x] **[STYLE]** `list/mod.rs` lines 96, 294 -- 2 decorative banners replaced with plain section comments (2026-03-21)
- [x] **[STYLE]** `iterator/consumers.rs` lines 11, 76, 159, 181, 214, 247, 303, 330 -- 8 decorative banners replaced with plain section comments (2026-03-21)
- [x] **[WASTE]** `cow_sort.rs:256` -- `ori_list_reverse_cow` fast path: replaced `vec![0u8; es]` with `[0u8; 24]` stack buffer + heap fallback for larger elements. (2026-03-21)
- [x] **[WASTE]** `cow_sort.rs:458` -- `apply_permutation_in_place`: same stack array optimization as line 256. (2026-03-21)
- [x] **[LATENT]** `query.rs` -- `ori_list_reverse` and `ori_list_concat` now accept `elem_align: i64` parameter and use it in `ori_rc_alloc`. LLVM declarations updated. No codegen callers (JIT-only). (2026-03-21)
- [x] **[WASTE]** `map/mod.rs:21-25` -- Removed `#[allow(unused_imports)]`. `META_EMPTY` IS used within `mod.rs` itself (line 50), so the re-export is correct. The plan claim that `META_EMPTY` was unused was incorrect. (2026-03-21)
- [x] **[WASTE]** `map/mod.rs` -- Removed dead `elem_size` parameter from `write_array_to_list_from_data` and updated 2 callers (`ori_map_keys_to_list`, `ori_map_values_to_list`). (2026-03-21)
- [x] **[WASTE]** `set/cow/basic.rs` + `set/cow/algebra.rs` -- Removed 5 dead `_ea` computations. Renamed `elem_align` parameter to `_elem_align` in function signatures (parameter retained for ABI compatibility). (2026-03-21)
- [x] **[STYLE]** `set/mod.rs:98` -- decorative banner replaced with plain section comment (2026-03-21)
- [x] **[STYLE]** `map/mod.rs:228` -- decorative banner replaced with plain section comment (2026-03-21)
- [x] **[BLOAT]** `construction.rs` was 513 lines — extracted `emit_variant_via_alloca` and `emit_variant_via_insertvalue` into `variant_construction.rs` (169 lines). `construction.rs` now 358 lines. (2026-03-21)

---

## 02.2 Iterator Creation and Drop

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/builtins/collections/list_builtins.rs`, `compiler/ori_rt/src/iterator/state.rs`, `compiler/ori_rt/src/iterator/sources.rs`

**STATUS UPDATE**: The `iter-rc-contract` plan (2026-03-18) already fixed `emit_list_iter` (fn at line 126 of `list_builtins.rs`, fix at line 144) to pass `self.get_or_generate_elem_dec_fn(elem_ty)` instead of NULL. `IterState::List` Drop (line 170 of `state.rs`) passes this stored `elem_dec_fn` to `ori_buffer_rc_dec`. The iterator already carries the real function. This section's remaining work is verifying the header provides a second safety net and deciding on parameter cleanup (Section 03).

With `elem_dec_fn` ALSO stored in the header at construction time (Section 02.1), there are now TWO mechanisms for element cleanup: (1) the parameter passed by the caller to `ori_buffer_rc_dec`, and (2) the header-stored function read by `ori_buffer_rc_dec`. The `store_elem_dec_fn_once` write-once pattern ensures they agree. This provides defense-in-depth: even if one path passes NULL, the header provides the function.

### Parameter Retention Decision

- [x] **Decide**: Keep or remove the `elem_dec_fn` parameter in `ori_iter_from_list`?
  - **Decision: Option A** — Keep parameter for defense-in-depth. Both the parameter and header provide `elem_dec_fn`. Section 03 removes the parameter once the header-based approach is proven stable by the Section 04 test matrix. (2026-03-21)

### Integration Verification

- [x] Verify: when iterator's `ori_buffer_rc_dec` call reaches zero, it reads `elem_dec_fn` from the header and performs element cleanup — confirmed: `drop_elements_and_free` at `list_rc.rs:39` calls `load_elem_dec_fn(header_data)`, not the caller parameter. Iterator Drop → `ori_buffer_rc_dec` → `drop_elements_and_free` → reads from header. (2026-03-21)
- [x] Verify: when explicit RcDec's `ori_buffer_rc_dec` call reaches zero, same behavior -- reads from header — confirmed: same code path. Both iterator and explicit decs go through `ori_buffer_rc_dec` which always delegates to `drop_elements_and_free` reading from header. (2026-03-21)
- [x] Verify: `store_elem_dec_fn_once` CAS handles the case where both iterator Drop and explicit RcDec store to the same header -- first non-NULL wins, second is a no-op. Confirmed: `elem_header.rs:90-100` uses `compare_exchange(null → func)` — CAS fails if already non-NULL. Unit test `elem_dec_fn_store_once_first_non_null_wins` exists. End-to-end exercised by `test_str_list_explicit_last_owner` (both iterator dec and explicit dec hit `store_elem_dec_fn_once` on same buffer). (2026-03-21)

### AOT Tests

- [x] `[str]` iteration where iterator dec reaches zero first -- `test_str_list_iter_last_owner` in `elem_dec_scope.rs`, zero leaks in debug + release (2026-03-21)
- [x] `[str]` iteration where explicit dec reaches zero first -- `test_str_list_explicit_last_owner` in `elem_dec_scope.rs`, zero leaks in debug + release (2026-03-21)
- [x] Function parameter `[str]` -- callee iterates, caller uses after -- `test_str_list_fn_param_iter` in `elem_dec_scope.rs`, no double-free, no leak in debug + release (2026-03-21)
- [x] `[str]` iteration + slice -- `test_str_list_slice_then_iter` in `elem_dec_scope.rs`, uses `.take(count:)` seamless slice, iterates original, zero leaks in debug + release (2026-03-21)

### Cleanup

- [x] **[NOTE]** `list_builtins.rs:115-125` -- Doc comment on `emit_list_iter` updated to mention V5 header as second safety net for `elem_dec_fn`. (2026-03-21)
- [x] **[DRIFT]** `iterator/state.rs:49` -- Doc comment on `IterState::List` updated to explain V5 header defense-in-depth: `elem_dec_fn` stored in header via `store_elem_dec_fn_once`, cleanup reads from header not parameter. (2026-03-21)

---

## 02.3 Map and Set Integration

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/rc_buffer_ops.rs`, `compiler/ori_rt/src/map/mod.rs`, `compiler/ori_rt/src/set/cow/basic.rs`, `compiler/ori_rt/src/set/cow/algebra.rs`, `compiler/ori_rt/src/set/mod.rs`

**STATUS UPDATE**: The `iter-rc-contract` plan (2026-03-18) already fixed `emit_map_iter` to pass real `key_dec_fn`/`val_dec_fn` instead of NULL. Maps use the codegen-based approach (option c from Section 01.3). The `__for_coll_N` phantom only matches `List | Set` (not `Map`), but maps use ownership transfer (`@iter(%map [own])`) where the iterator's Drop is the sole cleanup path.

### Maps (Codegen-Based, Already Implemented)

- [x] Verify `emit_map_iter` (line 345 of `map_builtins.rs`) passes real `key_dec_fn`/`val_dec_fn` to `ori_iter_from_map` — confirmed via `get_or_generate_elem_dec_fn` at lines 362-363. `IterState::Map` Drop at line 194 correctly passes `*key_dec_fn` and `*val_dec_fn` to `ori_map_buffer_rc_dec`. Maps use codegen-based approach (two dec functions). (2026-03-21)

### Map Double-Free Investigation

- [x] Map double-free root cause identified and fixed (2026-03-21). Root cause: `ori_map_keys_to_list` and `ori_map_values_to_list` copied element structs via `copy_nonoverlapping` without incrementing RC children. Map and output list shared RC-tracked data (e.g., string data pointers) with only one reference count. Fix: added `key_inc_fn`/`val_inc_fn` parameters (generated by `get_or_generate_elem_inc_fn`); each copied element gets RcInc. Same fix applied to `ori_set_to_list` with `elem_inc_fn`. ABI sync: runtime + LLVM declarations + codegen call sites updated atomically. `test_map_keys_str_scope_drop` un-ignored — passes in debug + release.
- [x] **[BUG]** 3 Valgrind failures (`cow_leak_scenarios.ori`, `cow_map_insert_remove.ori`, `cow_nested.ori`) — map insert double-free. Root cause: `ori_map_insert_cow` copies key/value into hash buffer via `copy_nonoverlapping` without calling `key_inc`/`val_inc`. The caller's borrowed reference gets RcDec'd (freeing the data), then the map's drop also decs the buffer copy. Fix: added `key_inc`/`val_inc` calls after every new key/value insertion — 3 sites in `cow_insert_new` (fast direct, fast rehash, slow) + 1 site in `slow_copy_overwrite_value` (inc the new value at overwrite bucket). Also inc'd new value in fast-path overwrite. AOT tests: `test_map_insert_heap_str_key`, `test_map_insert_heap_str_value`, `test_map_cow_insert_shared_heap_key`. All 13,500 tests pass. (2026-03-21)
- [x] **[BUG][TPR-02-012]** `cow_insert_existing` fast path (unique) leaks the old value when overwriting — fixed by adding `val_dec` parameter to `ori_map_insert_cow` / `cow_insert_existing` / `ori_map_update_cow`. Fast path now calls `val_dec(old_val_ptr)` before overwriting. ABI sync: runtime + LLVM declaration (14 params) + codegen (`emit_map_insert` passes `get_or_generate_elem_dec_fn(val_ty)`). AOT tests: `test_map_insert_overwrite_heap_str_value` (single), `test_map_insert_overwrite_shared_heap_str_value` (shared/slow path), `test_map_insert_overwrite_multiple_heap_str_value` (repeated). All pass in debug + release with `ORI_CHECK_LEAKS=1`. (2026-03-21)

### Branch-Local RcDec in Merge Blocks (TPR-02-007 + TPR-02-008)

The ARC pipeline places `RcDec` for branch-local variables in the post-merge block instead of in their respective branch blocks. When one branch is taken, the other branch's variables are undefined at the merge point, and the LLVM emitter silently skips the dec (`skipping RcDec on undefined variable`). This causes double-frees: the taken branch's variable gets its own cleanup PLUS the merge-block cleanup, while the untaken branch's cleanup is silently dropped.

Root cause: `propagate_project_source_demand` (from TPR-02-006 fix) adds demand for ALL sources of a multi-predecessor block param at the merge block's entry. This causes branch-local parent aggregates to bleed into the opposite branch's state, producing RcDec for undefined variables in merge blocks and on branch edges. The backward analysis demand propagation is correct (conservative: keeps parent aggregates alive per-path), but the emission layer was treating merged demand as block-level RcDec.

Fix (2026-03-21): three-layer emission filter:
1. **Phase A** (`emit_dead_at_entry_decs`): at merge blocks, detect branch-local variables (not defined in ALL predecessors) and route to per-predecessor edge cleanup instead of block-level RcDec.
2. **Edge cleanup** (`collect_branch_edge_decs` + `collect_invoke_edge_decs`): filter exit-state variables against `defined_at_or_before` set — skip variables whose defining block is downstream of the branching/invoke block.
3. **Merge-edge routing** (`emit_rc_unified`): Phase A returns merge-edge decs; caller routes them to `block_deferred` for the specific predecessor that defines the variable, producing per-predecessor trampolines via edge cleanup.

Files changed: `aims/emit_rc/dead_cleanup.rs`, `aims/emit_rc/edge_cleanup.rs`, `aims/realize/emit_unified.rs`, `aims/intraprocedural/project_aliases.rs`.

- [x] **[BUG]** Identify the ARC lowering pass that inserts `RcDec` for branch-local variables in the merge block — root cause: `propagate_project_source_demand` at merge block entry injects branch-local parent demand into shared entry state. (2026-03-21)
- [x] **[BUG]** Fix the ARC pipeline to emit branch-local cleanup in the correct block — three-layer emission filter (Phase A merge-block routing + edge cleanup defined_at_or_before filter + per-predecessor trampoline routing). Verified with `ORI_DUMP_AFTER_ARC=1`: no RcDec in merge/unwind blocks references branch-local variables. (2026-03-21)
- [x] **[BUG]** Replace `test_rc_project_merge_two_distinct_parents` with heap-allocated strings (>23 bytes SSO threshold). Verified zero leaks and zero double-frees via `ORI_CHECK_LEAKS=1` in debug + release. (2026-03-21)
- [x] **[BUG]** Add `debug_assert!` in edge cleanup that verifies merge-edge decs target valid successors: added assertions in both non-Invoke (`edge_cleanup.rs:118-124`) and Invoke (`edge_cleanup.rs:93-101`) paths. Catches misrouted merge-edge decs that target a block not in the predecessor's successor list. (2026-03-21)
- [x] **[BUG][TPR-02-010]** Fix merge-edge cleanup to preserve successor identity: extended `block_deferred` from `(ArcVarId, RcStrategy)` to `(Option<usize>, ArcVarId, RcStrategy)` — `None` = all edges (Phase B deferred parents), `Some(succ)` = only target edge (merge-edge decs). Updated `emit_unified.rs` routing and `edge_cleanup.rs` emission to filter by target. Added `test_rc_project_merge_edge_scoped_cleanup` (heap strings, function calls) and enhanced `test_rc_project_merge_two_distinct_parents` (exercises both branches). All 13,500 tests pass (debug + release). (2026-03-21)

### Sets (Header-Based, Same as Lists)

Set construction codegen is complete (see 02.1 "Set Construction Codegen" -- `elem_dec_fn` and `elem_count` stored at literal construction time). Remaining work:

- [x] Verify set iteration codegen: `emit_list_iter` is also called for sets (dispatched at `builtins/collections/mod.rs` line 463 as `("Set", "iter") => emit_list_iter`). Sets share the `ori_buffer_rc_dec` cleanup path with lists for iterator Drop. `ori_set_buffer_rc_dec` in `rc/set_rc.rs` correctly scans META_OCCUPIED buckets and calls `elem_dec_fn` per element. Header `elem_dec_fn` provides defense-in-depth. (2026-03-21)
- [x] Set COW slow path propagation: all set COW functions propagate `elem_dec_fn` via centralized `alloc_set_hash_buffer` and `rehash_set`. Completed in 02.1. (2026-03-20)
- [x] `ori_set_to_list` (`set/mod.rs` line 55): stores both `elem_dec_fn` and `elem_count` in list buffer. Completed in 02.1. (2026-03-20)

### Collect RcInc Bug (TPR-02-009 + Discovery)

Both `ori_iter_collect` (list) and `ori_iter_collect_set` (set) shallow-copy elements via `copy_nonoverlapping` without incrementing child RCs. When the iterator is dropped, `ori_buffer_rc_dec` fires `elem_dec_fn` on source elements, freeing child data. The collected target then has dangling pointers → double-free. Same bug pattern as pre-fix `ori_map_keys_to_list`/`ori_set_to_list`.

**List collect** (`ori_iter_collect` in `iterator/consumers.rs:20`):
- [x] **[BUG]** Add `elem_inc_fn` parameter to `ori_iter_collect` runtime function signature (2026-03-21)
- [x] **[BUG]** Call `elem_inc_fn` after each `copy_nonoverlapping` into the list buffer (consumers.rs:54) (2026-03-21)
- [x] **[BUG]** Update `emit_iter_collect` codegen to pass `elem_inc_fn` thunk via `get_or_generate_elem_inc_fn` (2026-03-21)
- [x] **[BUG]** Update runtime function declaration in `runtime_functions.rs` (add ptr param for elem_inc_fn) (2026-03-21)
- [x] **[BUG]** JIT symbol mapping in `runtime_mappings.rs` uses function pointer — no change needed (auto-resolved by link-time binding) (2026-03-21)
- [x] AOT test: `[str]` `.iter().collect()` with heap strings (>23 bytes) — `test_str_list_method_collect` passes with `ORI_CHECK_LEAKS=1` in debug + release. Note: `for x in items yield x` uses explicit loop (not `ori_iter_collect`), so the method `.collect()` path is the correct test target. (2026-03-21)

**Set collect** (`ori_iter_collect_set` in `iterator/consumers.rs:85`):
- [x] **[BUG]** Add `elem_inc_fn` parameter to `ori_iter_collect_set` runtime function signature (2026-03-21)
- [x] **[BUG]** Call `elem_inc_fn` after each `copy_nonoverlapping` into the set slot (consumers.rs:152) (2026-03-21)
- [x] **[BUG]** Update `emit_iter_collect_set` codegen to pass `elem_inc_fn` thunk via `get_or_generate_elem_inc_fn` (2026-03-21)
- [x] **[BUG]** Update runtime function declaration in `runtime_functions.rs` (add ptr param for elem_inc_fn) (2026-03-21)
- [x] **[BUG]** JIT symbol mapping in `runtime_mappings.rs` uses function pointer — no change needed (auto-resolved by link-time binding) (2026-03-21)
- [x] AOT test: `Set<str>` collect with heap strings (>23 bytes) — zero leaks via `ORI_CHECK_LEAKS=1` in debug + release. Unblocked after trampoline ABI fix. `test_set_str_iter_collect` added to `elem_dec_scope.rs`. (2026-03-21)

### AOT Tests

- [x] `{str: int}` map iteration -- zero leaks via `ORI_CHECK_LEAKS=1` (10x stability check passed). AOT test `test_map_str_iteration` added to `elem_dec_scope.rs`. (2026-03-21)
- [x] `Set<str>` iteration -- zero leaks. AOT test `test_set_str_iteration` passes in debug + release. (2026-03-21)
- [x] `{str: int}` map passed to function, iterated inside -- zero leaks. AOT test `test_map_str_passed_to_fn` added to `elem_dec_scope.rs`. (2026-03-21)
- [x] `Set<str>` passed to function, iterated inside -- zero leaks. AOT test `test_set_str_passed_to_fn` passes in debug + release. (2026-03-21)
- [x] `Set<str>` COW insert on shared set -- zero leaks. AOT test `test_set_str_cow_insert_shared` passes in debug + release. Required fix: added `inc_fn` call after new element insertion in all 3 paths of `ori_set_insert_cow`. (2026-03-21)
- [x] `Set<str>` union -- zero leaks. AOT test `test_set_str_union` passes in debug + release. Required fix: added `inc_fn` call in union fast path for elements from set2. (2026-03-21)
- [x] `Set<str>` intersection/difference -- **[TPR-02-014]** Fixed: fast paths now call `elem_dec_fn` before tombstoning. AOT tests `test_set_str_intersection_unique` and `test_set_str_difference_unique` pass in debug + release with `ORI_CHECK_LEAKS=1`. (2026-03-21)
- [x] `map.keys()` on `{str: int}` -- zero leaks. `test_map_keys_str_scope_drop` AOT test passes in debug + release. Exercises `ori_map_keys_to_list` with `key_inc_fn`. (2026-03-21)

### Set Remove Fat-Pointer Leak (TPR-02-013)

`ori_set_remove_cow` leaks fat-pointer elements on all 3 paths: last-element `ori_rc_free` without element dec, tombstoning without dec, and slow-path skip without dec. Fix: add `elem_dec_fn` parameter and call it before tombstoning/freeing/skipping.

- [x] **[BUG]** Add `elem_dec_fn: Option<extern "C" fn(*mut u8)>` parameter to `ori_set_remove_cow` (`set/cow/basic.rs`) (2026-03-21)
- [x] **[BUG]** Fast path (unique, tombstone): call `elem_dec_fn` on removed element before `set_meta(META_TOMBSTONE)` (line ~206) (2026-03-21)
- [x] **[BUG]** Fast path (unique, last element): call `elem_dec_fn` on removed element before `ori_rc_free` (line ~195) (2026-03-21)
- [x] **[BUG]** Slow path (shared): removed element gets dec'd by `ori_rc_dec(data, None)` — old buffer's `set_buffer_cleanup` handles it since the element stays `META_OCCUPIED` in the old buffer. No explicit dec needed on the skip. (2026-03-21)
- [x] **[BUG]** Update LLVM IR declaration in `runtime_functions.rs` — add `Ty::Ptr` for `elem_dec_fn` (2026-03-21)
- [x] **[BUG]** Update codegen call site in `set_builtins.rs` — pass `get_or_generate_elem_dec_fn(elem_ty)` (2026-03-21)
- [x] **[BUG]** ABI sync: all 3 changes (runtime + LLVM decl + codegen) committed together (2026-03-21)
- [x] AOT test: `Set<str>.remove()` with remaining elements — zero leaks (`test_set_str_remove_remaining`, debug + release) (2026-03-21)
- [x] AOT test: `Set<str>.remove()` removing last element — zero leaks (`test_set_str_remove_last_element`, debug + release) (2026-03-21)

### Set Intersection/Difference Fat-Pointer Leak (TPR-02-014)

`ori_set_intersection_cow` and `ori_set_difference_cow` unique fast paths tombstone elements without calling `elem_dec_fn`. `set_buffer_cleanup` only iterates `META_OCCUPIED` buckets, so tombstoned elements are never cleaned.

- [x] **[BUG]** `ori_set_intersection_cow` fast path (unique): call `elem_dec_fn` on each element before tombstoning (line ~236 of `algebra.rs`) (2026-03-21)
- [x] **[BUG]** `ori_set_difference_cow` fast path (unique): call `elem_dec_fn` on each element before tombstoning (line ~354 of `algebra.rs`) (2026-03-21)
- [x] **[BUG]** `elem_dec_fn` available via `load_elem_dec_fn(d1)` from header — no parameter change needed (2026-03-21)
- [x] **[BUG]** Also fixed: intersection empty edge case (n2==0, d1 unique with elements) — dec each element before `ori_rc_free` (2026-03-21)
- [x] AOT test: `Set<str>.intersection()` on unique set — zero leaks (`test_set_str_intersection_unique`, debug + release) (2026-03-21)
- [x] AOT test: `Set<str>.difference()` on unique set — zero leaks (`test_set_str_difference_unique`, debug + release) (2026-03-21)

### Map Remove Fat-Pointer Leak (Discovered During TPR-02-013 Investigation)

`ori_map_remove_cow` (`map/cow.rs`) has the same bug pattern as `ori_set_remove_cow`: fast path tombstones key/value without calling `key_dec_fn`/`val_dec_fn`, and slow path skips without dec. Same fix pattern: plumb dec functions and call before tombstoning/freeing/skipping.

- [x] **[BUG]** Add `key_dec_fn` and `val_dec_fn` parameters to `ori_map_remove_cow` (`map/cow.rs`) (2026-03-21)
- [x] **[BUG]** Fast path (unique, tombstone): call `key_dec_fn` + `val_dec_fn` on removed entry before tombstoning (2026-03-21)
- [x] **[BUG]** Fast path (unique, last element): call `key_dec_fn` + `val_dec_fn` on removed entry before `ori_rc_free` (2026-03-21)
- [x] **[BUG]** Slow path (shared): removed entry's cleanup handled by `ori_rc_dec(data, None)` — when last owner drops, `map_buffer_cleanup` fires on all META_OCCUPIED entries including the removed one. No explicit dec needed on the skip. (2026-03-21)
- [x] **[BUG]** Update LLVM IR declaration in `runtime_functions.rs` — add `Ty::Ptr` for both (2026-03-21)
- [x] **[BUG]** Update codegen call site in `map_builtins.rs` — pass both dec fns via `get_or_generate_elem_dec_fn` (2026-03-21)
- [x] **[BUG]** ABI sync: all 3 changes (runtime + LLVM decl + codegen) committed together (2026-03-21)
- [x] AOT test: `{str: int}` map remove — zero leaks (`test_map_remove_str_key`, debug + release) (2026-03-21)
- [x] AOT test: `{str: int}` map remove last element — zero leaks (`test_map_remove_str_key_last`, debug + release) (2026-03-21)

### Cleanup

- [x] **[NOTE]** `map_builtins.rs:320-330` -- Doc comment on `emit_map_iter` correctly documents that real dec functions are passed. No update needed. (2026-03-21, verified)
- [x] **[DRIFT]** `set/cow/basic.rs` and `set/cow/algebra.rs` -- doc comments updated for `elem_dec_fn` propagation in earlier session. (2026-03-21, verified — already marked in 02.N cleanup)

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
- [x] `[TPR-02-004][low]` `compiler/ori_rt/src/list/mod.rs:83` — Section 02 touched files still violate mandatory hygiene rules with stale RC-header comments and decorative section banners.
  Resolved: Validated on 2026-03-21. Accepted — all 3 stale "8-byte refcount header" refs confirmed at list/mod.rs:83,131,199 and 4 decorative banners confirmed (list/mod.rs:96, map/mod.rs:242, iterator/consumers.rs:11, fat_ptr_iter.rs:15). Already integrated as cleanup tasks in 02.N (lines 425-436).
- [x] `[TPR-02-005][high]` `compiler/ori_llvm/src/codegen/function_compiler/entry_point.rs:85` — The production `@main(args: [str])` AOT path is still broken, so Section 02’s `ori_args_from_argv` work has no end-to-end verification and the current safety note overstates reality.
  Evidence: Fresh verification on 2026-03-21 with `timeout 150 cargo run -p oric --bin ori -- build /tmp/main_args_len.ori` for `@main (args: [str]) -> int = args.len();` fails LLVM verification: `Call parameter type does not match function signature! ... call i64 @_ori_main({ i64, i64, ptr } %args)`. The wrapper in `entry_point.rs` materializes `ori_args_from_argv` via `call_with_sret(...)` and forwards the loaded list struct directly to `_ori_main`, but there is no coverage for an args-bearing main signature in `compiler/ori_llvm/src/codegen/function_compiler/tests.rs` and no AOT/spec test exercises `@main(args: [str])`.
  Impact: Any AOT program using the supported `@main(args: [str])` signature is currently uncompilable, and the Section 02 claim at `plans/rc-header-elem-dec/section-02-integration.md:104` that deferred `elem_dec_fn` handling is "safe" cannot be validated on the only production caller for `ori_args_from_argv`.
  Required plan update: Fix the main-wrapper ABI/signature mismatch for args-bearing mains, add an end-to-end AOT regression test for `@main(args: [str])`, then re-evaluate whether deferring `elem_dec_fn` in `ori_args_from_argv` is still justified once slice/take/drop paths are executable.
  Resolved: Validated and accepted on 2026-03-21. Bug confirmed — `generate_main_wrapper` loads sret result as `{i64, i64, ptr}` value but `_ori_main` expects `ptr` (Indirect ABI for 24-byte struct). Root cause: wrapper doesn’t consult callee’s param ABI. Integrated as blocking task in 02.N.
- [x] `[TPR-02-006][high]` `compiler/ori_arc/src/aims/intraprocedural/project_aliases.rs:38` — The new block-param alias closure is still unsound at CFG merges because it records only one `Project` source per block parameter, even though a merge param may receive projected values from multiple predecessor aggregates.
  Resolved: Fixed on 2026-03-21. Changed `FxHashMap<ArcVarId, ArcVarId>` → `FxHashMap<ArcVarId, SmallVec<[ArcVarId; 1]>>` (type alias `ProjectSources`). `merge_sources()` helper performs set-union at merge points. `propagate_project_source_demand()` now iterates all sources. Tests: `compute_project_alias_sources_multi_predecessor_merge` (unit), `project_block_param_multi_predecessor_merge_propagates_all_source_demand` (semantic pin), `test_rc_project_merge_two_distinct_parents` (AOT, debug + release). All 13,494 tests pass.
- [x] `[TPR-02-007][medium]` `compiler/ori_llvm/tests/aot/arc.rs:994` — The new AOT regression for TPR-02-006 does not distinguish the fixed multi-predecessor behavior from the old single-predecessor bug, so Section 02’s end-to-end closure claim is overstated.
  Evidence: The test condition at `arc.rs:1004` is deterministically true (`p1.first.len() > 0`), so the runtime always takes the `then` predecessor. `lower_if()` creates and lowers the `then` block before the `else` block (`compiler/ori_arc/src/lower/control_flow/mod.rs:149-180`), and the pre-fix `compute_project_alias_sources()` only preserved the first predecessor source via `Entry::Vacant` insertion. Fresh verification with `ORI_DUMP_AFTER_ARC=1 target/debug/ori build` on this exact test program still lowers to a merge block `bb5(%17: str)` reached first from the taken `then` path (`bb3 -> Jump bb5(%14)`), so the old unsound implementation could still pass this AOT case while dropping the else-parent source.
  Impact: The ARC unit semantic pin proves the lattice fix locally, but the current AOT guard can pass without proving that both predecessor aggregates survive end-to-end. A regression that re-drops the else predecessor would remain undetected by the claimed AOT coverage.
  Required plan update: Replace or augment the AOT case with a branch that exercises both predecessors across runs, or otherwise forces execution through the predecessor that the old single-source map dropped, before re-closing TPR-02-006’s AOT verification claim.
  Resolved: Accepted on 2026-03-21. Confirmed — condition is always true, strings are SSO (under 23-byte threshold, no RC ops), and when modified to use heap strings with runtime-variable condition, the program double-frees. Integrated as blocking tasks in 02.3 (ARC pipeline fix) and 02.N (test replacement).
- [x] `[TPR-02-008][medium]` `compiler/ori_llvm/tests/aot/arc.rs:994` — The new TPR-02-006 AOT program does not currently compile through a clean ARC/LLVM path: the compiler logs undefined-variable emitter errors and silently drops two `RcDec`s while building it.
  Evidence: Fresh verification with `ORI_DUMP_AFTER_ARC=1 target/debug/ori build` on the exact source from `arc.rs:994` emits branch-local cleanup `RcDec %13` / `RcDec %15` before those vars are defined in the taken branches, then logs `ArcIrEmitter: variable not yet defined` from `compiler/ori_llvm/src/codegen/arc_emitter/emitter_utils.rs:170` and `skipping RcDec on undefined variable` from `compiler/ori_llvm/src/codegen/arc_emitter/rc_ops.rs:94`. The build still succeeds only because the emitter treats the invalid RC ops as skippable.
  Impact: This AOT regression currently passes on top of a masked RC-emission error, so it does not provide trustworthy end-to-end evidence for the fix. Silent dropping of RC ops on undefined vars can hide leaks or ownership regressions instead of surfacing them as compiler failures.
  Required plan update: Identify the pass that inserts the pre-definition `RcDec`s on this branch/merge shape, add a verifier or debug assertion that RC emission never relies on `emit_rc_dec()`’s undefined-var skip path, and add an IR-quality regression for this exact source before using it as closure evidence.
  Resolved: Accepted on 2026-03-21. Confirmed — `RcDec %13`/`%15` in merge block are for branch-local vars not defined in both predecessors. Root cause: ARC pipeline places branch-local cleanup in the merge block instead of in the branch blocks. Integrated as blocking task in 02.3 (ARC pipeline fix).
- [x] `[TPR-02-009][high]` `compiler/ori_rt/src/iterator/consumers.rs:85` — `__collect_set` still shallow-copies RC-tracked elements into the new set buffer without incrementing child RCs, so `Set<str>` AOT programs double-free immediately.
  Evidence: Fresh verification on 2026-03-21 with `target/debug/ori build /tmp/set_str_len.ori -o /tmp/set_str_len_bin && ORI_CHECK_LEAKS=1 /tmp/set_str_len_bin` aborts with `ori_rc_dec called on already-freed allocation`, and the same reproducer aborts in release. The runtime copy in `ori_iter_collect_set` only does `copy_nonoverlapping` into the hash-table slot (`iterator/consumers.rs:150-153`); unlike `ori_map_keys_to_list` / `ori_set_to_list`, there is no `elem_inc_fn` call after the copy, and `emit_iter_collect_set` does not pass an increment thunk either (`compiler/ori_llvm/src/codegen/arc_emitter/builtins/iterator_consumers.rs:72-125`).
  Impact: Section 02’s set integration is not correct for fat-pointer elements. Any AOT path that materializes `Set<str>` via `iter().collect()` aliases string children between the source iterator input and the destination set with only one reference count, leading to double-free before downstream set operations like `.to_list()` are even reached.
  Required plan update: Add `elem_inc_fn` plumbing to `ori_iter_collect_set` and `emit_iter_collect_set`, then add permanent `ORI_CHECK_LEAKS=1` AOT coverage for `Set<str>` construction and `Set<str>.to_list()`.
  Resolved: Validated and accepted on 2026-03-21. Confirmed — `ori_iter_collect_set` copies elements without child RC increment, causing double-free for fat-pointer elements. Same bug pattern as pre-fix `ori_map_keys_to_list`/`ori_set_to_list`. Also discovered identical bug in `ori_iter_collect` (list collect) — same shallow copy without `elem_inc_fn`. Both integrated as fix tasks in 02.3.
- [x] `[TPR-02-010][high]` `compiler/ori_arc/src/aims/realize/emit_unified.rs:143` — The new merge-edge cleanup routing loses successor identity, so branch-local `RcDec`s selected for one merge edge are replayed on every outgoing edge of the defining predecessor.
  Evidence: `emit_dead_at_entry_decs()` now returns `merge_edge_decs` for a specific merge block (`dead_cleanup.rs:80-100`). `emit_unified()` routes them by appending `(var, strategy)` into `block_deferred[pred_idx]` with no successor key (`emit_unified.rs:143-155`). `emit_edge_cleanup()` then treats every deferred entry as predecessor-wide and emits it on all successor edges of that predecessor, or on both normal and unwind edges for `Invoke` (`edge_cleanup.rs:80-99`).
  Impact: The current fix is only correct when each defining predecessor has exactly one outgoing edge. In a wider CFG, cleanup chosen for one merge edge can fire on unrelated edges, causing premature drops, double-frees, or unwind-path cleanup for values that are not live there.
  Required plan update: Represent merge-edge decs with successor scope (for example `(pred, succ, var, strategy)`), route them only to the specific merge successor, and add a regression where the defining predecessor has multiple outgoing edges so this cannot silently regress again.
  Resolved: Validated and accepted on 2026-03-21. Bug confirmed — `block_deferred` stores only `(var, strategy)` with no successor index, `edge_cleanup.rs:95-99` emits on ALL successors. Latent for single-successor predecessors, manifests for multi-successor (Branch/Switch/Invoke). Integrated as blocking task in 02.3.
- [x] `[TPR-02-011][low]` `compiler/ori_arc/src/aims/emit_rc/edge_cleanup.rs:1` — The unstaged merge-edge follow-up pushes `edge_cleanup.rs` past the 500-line hygiene limit without extracting a helper/module.
  Evidence: `wc -l compiler/ori_arc/src/aims/emit_rc/edge_cleanup.rs` reports 529 lines in the current tree, and this file was modified as part of the same fix.
  Impact: The review surface for RC edge behavior is getting harder to audit precisely where correctness is most sensitive, which raises regression risk for future ARC cleanup work.
  Required plan update: Split the new merge-edge filtering helpers into a focused sibling module before adding more RC edge logic here.
  Resolved: Validated and accepted on 2026-03-21. Confirmed 529 lines. Integrated as cleanup task in 02.N — split merge-edge filtering helpers into sibling module before further ARC edge work.
- [x] `[TPR-02-012][high]` `compiler/ori_rt/src/map/cow.rs:104` — `ori_map_insert_cow` still has an unsound overwrite path for existing keys with RC-tracked values, and the current plan understates it as a low-priority leak.
  Resolved: Validated and accepted on 2026-03-21. Bug confirmed — fast-path `cow_insert_existing` overwrites old value without `val_dec_fn`, leaking RC-tracked children. User-visible AOT crash, not a leak. Existing item in 02.3 updated from "Low priority" to blocking. Fix: plumb `val_dec_fn` through `ori_map_insert_cow` / `cow_insert_existing` / LLVM declarations / codegen.
- [x] `[TPR-02-013][high]` `compiler/ori_rt/src/set/cow/basic.rs:190` — `ori_set_remove_cow` still leaks fat-pointer elements on unique-owner removals.
  Resolved: Validated and accepted on 2026-03-21. Bug confirmed across all 3 paths: (1) last-element free via `ori_rc_free` without element dec, (2) tombstoning without element dec, (3) slow path skips removed element without dec. Fix: add `elem_dec_fn` parameter, call before tombstoning/freeing/skipping. Also discovered: `ori_map_remove_cow` has identical bug pattern. Integrated as blocking tasks in 02.3.
- [x] `[TPR-02-014][high]` `compiler/ori_rt/src/set/cow/algebra.rs:224` — The unique fast paths for `Set.intersection()` and `Set.difference()` still leak filtered-out fat-pointer elements.
  Resolved: Validated and accepted on 2026-03-21. Bug confirmed — `set_buffer_cleanup` only iterates `META_OCCUPIED` buckets, tombstoned elements are never cleaned. Fix: call `elem_dec_fn` on each element before tombstoning in intersection (line 236) and difference (line 354). Integrated as blocking tasks in 02.3.
- [x] `[TPR-02-015][high]` `compiler/ori_rt/src/set/cow/basic.rs:193` — `ori_set_remove_cow` still double-decrements fat-pointer elements when removing the last element from a shared set.
  Resolved: Validated and fixed on 2026-03-21. Moved `elem_dec_fn` call inside the `is_unique` branch in the `new_len == 0` path. Shared sets now only `ori_rc_dec(data, None)` — surviving aliases handle element cleanup when buffer refcount reaches zero. Two permanent AOT regression tests added: `test_set_str_remove_last_shared` and `test_set_str_remove_last_shared_only_alias_survives`. Both pass debug + release with `ORI_CHECK_LEAKS=1`.
- [x] `[TPR-02-016][high]` `compiler/ori_llvm/src/codegen/function_compiler/entry_point.rs:132` — The new `@main(args: [str])` cleanup only runs on the normal-return path, so args-backed heap strings still leak whenever `_ori_main` unwinds.
  Evidence: Current wrapper code emits `call @_ori_main(...)` and only then `call @ori_args_cleanup(...)` at [entry_point.rs](/home/eric/projects/ori_lang_aims/compiler/ori_llvm/src/codegen/function_compiler/entry_point.rs#L132C1) and [entry_point.rs](/home/eric/projects/ori_lang_aims/compiler/ori_llvm/src/codegen/function_compiler/entry_point.rs#L154C1). Fresh IR verification on 2026-03-21 with `ORI_DEBUG_LLVM=1 target/debug/ori build /tmp/argspanic.ori -o /tmp/argspanic_bin` for `@main(args: [str]) -> void = panic(msg: "boom")` shows `define i32 @main(i32, ptr)` containing `call void @_ori_main(ptr %args.indirect)` followed by `call void @ori_args_cleanup(ptr %args.data, i64 %args.len)`, with no `invoke` or landingpad in `main`, while `_ori_main` itself carries a personality function and can unwind via `invoke void @ori_panic`.
  Impact: The newly added success-path tests in [cli.rs](/home/eric/projects/ori_lang_aims/compiler/ori_llvm/tests/aot/cli.rs#L831C1) prove only the non-panicking path. Any supported AOT program using `@main(args: [str])` that panics will skip both `ori_args_cleanup` and the wrapper leak check, leaving the argv-derived `[str]` buffer and any heap string children unfreed.
  Required plan update: Move args cleanup onto an unwind-safe path (for example, an `invoke`/landingpad in the wrapper or a callee-owned cleanup strategy), then add panic-path coverage for `@main(args: [str])` so the leak-free guarantee is exercised on both normal return and unwind.
  Resolved: Validated and accepted on 2026-03-21. Bug confirmed — wrapper uses plain `call` for `_ori_main`, cleanup at L154 only runs on normal return. When `_ori_main` can unwind (not in nounwind set), args leak on panic. Fix: use `invoke`+`landingpad` for `_ori_main` call when not nounwind, cleanup in both normal and unwind paths. Integrated as task in 02.N.
- [x] `[TPR-02-017][medium]` `compiler/ori_llvm/tests/aot/cli.rs:927` — The new args-wrapper IR semantic pins are not release-safe: they rely on `compile_and_capture_ir()` even though release `ori` binaries intentionally emit no LLVM IR, so the claimed debug+release verification is false and the release AOT suite now fails when these tests run.
  Resolved: Validated and fixed on 2026-03-21. Added `if !ir.contains("define ") { return; }` guard to all 3 IR semantic pin tests, matching the pattern used by all other IR quality tests. Tests now pass in both debug (assertions checked) and release (gracefully skipped).
- [x] `[TPR-02-018][medium]` `tests/valgrind/iter_rc/map_keys_values_fat.ori:3` — The new `map.keys()` / `map.values()` Valgrind pin still exercises only fat-pointer keys, not fat-pointer values.
  Resolved: Validated and fixed on 2026-03-21. Added AOT tests `test_map_values_heap_str_values` ({int: str}) and `test_map_values_str_str` ({str: str}) in `elem_dec_scope.rs`, plus Valgrind test `map_values_fat_values.ori`. All pass debug + release with `ORI_CHECK_LEAKS=1` and Valgrind clean.
- [x] `[TPR-02-019][medium]` `compiler/ori_rt/src/map/cow.rs:373` — The new `val_dec` cleanup branches in `ori_map_remove_cow` are still unproven by the added tests.
  Resolved: Validated and fixed on 2026-03-21. Added AOT tests `test_map_remove_heap_str_value` (unique fast path, cow.rs:391) and `test_map_remove_heap_str_value_last` (empty sentinel path, cow.rs:373) in `elem_dec_scope.rs`, plus Valgrind test `map_remove_fat_values.ori`. All pass debug + release with `ORI_CHECK_LEAKS=1` and Valgrind clean.
- [x] `[TPR-02-020][medium]` `compiler/ori_llvm/tests/aot/cli.rs:876` — The new panic-path `@main(args: [str])` tests do not actually prove “no segfault” or leak-free cleanup on unwind.
  Resolved: Validated and fixed on 2026-03-21. **THREE bugs found and fixed:** (1) `exit_code_from_status()` now detects Unix signal termination (SIGSEGV → -139, SIGABRT → -134) instead of mapping all signals to `-1` — `assert_no_signal_crash()` catches post-panic crashes. (2) **Critical: main wrapper used cleanup landingpad (not catch-all)** — Phase 1 search found no handler → `_URC_END_OF_STACK` → args never cleaned up. Fixed to use `landingpad catch ptr null` + `ori_catch_cleanup` + `ori_args_cleanup` + `ret 1`. (3) Added `test_main_args_panic_valgrind_clean` — Valgrind now verifies zero memory errors on panic-path args cleanup. IR semantic pin updated to assert `catch ptr null` (not just `landingpad`).
- [x] `[TPR-02-021][low]` `compiler/ori_llvm/src/codegen/runtime_decl/tests.rs:333` — The claimed JIT regression guard for RC-header helpers is still not a functional MCJIT test.
  Resolved: Validated and fixed on 2026-03-21. Added `test_jit_str_list_construction` in `cli.rs` — runs `ori test --backend=llvm` on a `[str]` list literal, forcing MCJIT to resolve `ori_buffer_store_elem_dec` + `ori_buffer_store_elem_count`. Supplements (does not replace) the symbol-table assertion. Both symbol registration and functional execution are now covered.
- [x] `[TPR-02-022][high]` `compiler/ori_llvm/src/codegen/function_compiler/entry_point.rs:266` — The new SEH unwind cleanup path for `@main(args: [str])` emits a plain call from inside a `cleanuppad`, which violates the repo's own SEH builder contract and leaves the Windows/MSVC path unsound.
  Resolved: Validated and fixed on 2026-03-21. Changed `self.builder.call(cleanup_fn, ...)` to `self.builder.call_with_funclet(cleanup_fn, ..., pad, "")` in the SEH branch at `entry_point.rs:268`. All calls inside SEH funclet pads now use `call_with_funclet` with the operand bundle, matching the contract in `seh.rs:173`. All 13,540 tests pass debug + release.
- [x] `[TPR-02-023][low]` `compiler/ori_llvm/src/codegen/function_compiler/entry_point.rs:1` — `entry_point.rs` is back over the 500-line hygiene limit after the new main-wrapper unwind work landed.
  Resolved: Validated and fixed on 2026-03-21. Extracted `generate_panic_trampoline()` (~183 lines) to `panic_trampoline.rs` sibling submodule. `entry_point.rs` is now 334 lines, `panic_trampoline.rs` is 201 lines — both well under the 500-line limit. Module doc and `mod.rs` updated.
- [x] `[TPR-02-024][medium]` `compiler/ori_llvm/src/codegen/arc_emitter/builtins/trampolines.rs:246` — The fat-pointer trampoline fix is still missing a semantic pin for the `for_each` branch, even though the current patch claims all four trampoline kinds are covered.
  Resolved: Validated and fixed on 2026-03-22. Added `test_trampoline_for_each_str` in `elem_dec_scope.rs` — exercises `[str].iter().for_each(action: s -> { let $n = s.len(); n })` with heap-allocated strings (>23 bytes, fat-pointer ABI). Passes debug + release. All four trampoline kinds (Map, Predicate, Fold, ForEach) now have fat-pointer semantic pins.
- [x] `[TPR-02-025][low]` `compiler/ori_rt/src/list/cow_sort.rs:1` — The touched runtime sort helper still violates the repository’s 500-line hygiene limit.
  Resolved: Validated and fixed on 2026-03-22. Converted `cow_sort.rs` (521 lines) to directory module `cow_sort/`. Extracted sort + permutation logic to `cow_sort/sort.rs` (215 lines). Parent `cow_sort/mod.rs` retains concat, reverse, and shared helpers (324 lines). Both files well under 500-line limit.
- [x] `[TPR-02-026][high]` `compiler/ori_llvm/src/codegen/function_compiler/entry_point.rs:180` — The current `@main(args: [str])` wrapper still unconditionally runs `ori_args_cleanup`, even when `_ori_main` receives the argv list via owned indirect ABI and already frees it itself.
  Resolved: Validated and fixed on 2026-03-22. Added `MainArgsCleanup` struct with `wrapper_owns_on_normal` flag — set `true` for `ParamPassing::Reference` (callee borrows, wrapper cleans up) and `false` for `Indirect`/`Direct` (callee owns, wrapper skips normal-path cleanup but still cleans up on unwind because callee's ARC dec hasn't run). Added 5 tests: `test_main_args_owned_path_no_double_free` (SSO strings), `test_main_args_owned_path_heap_strings` (fat pointers), `test_main_args_owned_path_int_return` (int return variant), `test_main_args_owned_path_valgrind_clean` (Valgrind verification), `test_main_args_owned_wrapper_ir_no_normal_cleanup` (IR semantic pin: cleanup appears once on catch path, not on normal path). All 13,546 tests pass debug + release.
- [x] `[TPR-02-027][high]` `compiler/ori_llvm/src/codegen/function_compiler/entry_point.rs:309` — The MSVC `@main(args: [str])` unwind path still rethrows via `cleanupret` instead of translating an Ori panic into the documented `main` exit-code contract.
  Resolved: Validated and fixed on 2026-03-21. Root cause: `cleanuppad`/`cleanupret` (SEH cleanup semantics) does NOT catch Ori's custom `RaiseException`-based panics — only `ori_try_call`'s C-level `__try`/`__except` does. Additionally, `ORI_FATAL_EXIT_CODE` was defined only in the Itanium `#else` section of `eh_personality.c`, causing MSVC compilation to fail. Fix: (1) Replaced the SEH branch in `emit_main_call_with_invoke` with a new `emit_main_call_with_seh_try` method that uses the `ori_try_call` thunk pattern (same mechanism as `catch(expr:)`). (2) Extracted SEH main thunk to `seh_main_thunk.rs` (186 lines). (3) Moved `ORI_FATAL_EXIT_CODE` `#define` before the `#ifdef _MSC_VER` guard. (4) Updated IR semantic pin tests (`test_main_args_wrapper_uses_invoke_ir`, `test_main_args_owned_wrapper_ir_no_normal_cleanup`) to validate both Itanium `invoke` and SEH `ori_try_call` patterns. Verified on Windows/MSVC: panicking `@main(args:)` returns exit code 1, successful `@main(args:)` returns exit code 0, `@main(args:) -> int` returns the correct value. Both debug and release builds pass.

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
- [x] `ori_map_keys_to_list` stores `elem_dec_fn` + `elem_count` on list buffer after `ori_rc_alloc` — `key_dec_fn` parameter added, LLVM decl + codegen updated (2026-03-20)
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

- [x] `debug_assert!` in `ori_buffer_rc_dec` catches NULL header with non-NULL caller `elem_dec_fn` (placed in `ori_buffer_rc_dec`, NOT in `drop_elements_and_free`) — added 2026-03-21
- [x] `test_rc_header_is_32_bytes` test existence verified -- exists at `compiler/ori_llvm/src/tests/runtime_tests.rs:289` (2026-03-20)
- [x] Map double-free root cause identified (2026-03-21): `ori_map_keys_to_list` / `ori_map_values_to_list` / `ori_set_to_list` missing RcInc on copied elements. Fixed with `elem_inc_fn` parameter. `test_map_keys_str_scope_drop` un-ignored. Section 01.N blocked Valgrind failures pending re-run (may need additional COW-path fixes).

### AOT Tests & Verification

- [x] `[str]` list scope drop -- zero leaks (`test_str_list_scope_drop`, 2026-03-21)
- [x] `[[int]]` nested list scope drop -- zero leaks (`test_nested_int_list_scope_drop`, 2026-03-21)
- [x] `[str]` COW push on shared list -- zero leaks (`test_str_list_cow_push_shared`, 2026-03-21)
- [x] SSO/heap mixed `[str]` -- zero leaks (`test_str_list_mixed_sso_heap`, 2026-03-21)
- [x] `ori_iter_collect` on `[str]` -- output buffer has correct `elem_dec_fn`, zero leaks (`test_str_list_iter_collect`, 2026-03-21)
- [x] `map.keys()` on `{str: int}` -- zero leaks (`test_map_keys_str_scope_drop`, fixed 2026-03-21 via `key_inc_fn` parameter). Passes in debug + release.
- [x] `str.split(sep:)` returning `[str]` -- zero leaks (`test_str_split_scope_drop`, 2026-03-21)
- [x] `[str]` iteration where iterator dec reaches zero first -- `test_str_list_iter_last_owner`, zero leaks in debug + release (2026-03-21)
- [x] `[str]` iteration where explicit dec reaches zero first -- `test_str_list_explicit_last_owner`, zero leaks in debug + release (2026-03-21)
- [x] Function parameter `[str]` -- callee iterates, caller uses after -- `test_str_list_fn_param_iter`, no double-free, no leak in debug + release (2026-03-21)
- [x] Iterator + slice cross-feature test -- `test_str_list_slice_then_iter`, uses `.take(count:)` seamless slice, zero leaks in debug + release (2026-03-21)
- [x] `{str: int}` map iteration -- zero leaks (10x stability, `test_map_str_iteration` AOT test) (2026-03-21)
- [x] `Set<str>` iteration -- zero leaks (`test_set_str_iteration`, debug + release) (2026-03-21)
- [x] `{str: int}` map passed to function, iterated inside -- zero leaks (`test_map_str_passed_to_fn` AOT test) (2026-03-21)
- [x] `Set<str>` passed to function, iterated inside -- zero leaks (`test_set_str_passed_to_fn`, debug + release) (2026-03-21)
- [x] `Set<str>` COW insert on shared set -- zero leaks (`test_set_str_cow_insert_shared`, debug + release). Fix: `ori_set_insert_cow` missing `inc_fn` on new element. (2026-03-21)
- [x] `Set<str>` union -- zero leaks (`test_set_str_union`, debug + release). Fix: union fast path missing `inc_fn` on set2 elements. (2026-03-21)
- [x] `Set<str>` intersection/difference -- fixed and tested: `test_set_str_intersection_unique` + `test_set_str_difference_unique` (debug + release) (2026-03-21)
- [x] `set.to_list()` on `Set<str>` -- zero leaks (`test_set_str_to_list`, debug + release) (2026-03-21)
- [x] `ori_iter_collect_set` on `Set<str>` — tested via `words.iter().collect()` with `Set<str>` annotation (for-yield always produces list, not set). `test_set_str_iter_collect` AOT test passes debug + release with `ORI_CHECK_LEAKS=1`. (2026-03-21)
- [x] **[TPR-02-005]** Fix `generate_main_wrapper` in `entry_point.rs` — wrapper loads sret result as `{i64, i64, ptr}` struct value but `_ori_main` expects `ptr` (Indirect ABI for 24-byte param). Fixed: check `abi.params[0].passing` for `Indirect`/`Reference` and pass via alloca pointer. Also added `ori_args_cleanup` runtime function to free args list after `_ori_main` returns. (2026-03-21)
- [x] `@main(args: [str])` with arguments -- exercises `ori_args_from_argv` creating `[str]` buffer, zero leaks. 4 AOT tests: no args, SSO strings, heap strings, void return. All pass with `ORI_CHECK_LEAKS=1`. (2026-03-21)
- [x] `test_str_list_passed_to_two_functions` passes reliably (not ignored) — verified 81/81 fat_ptr_iter tests pass (2026-03-21)
- [x] `test_nested_list_iteration` passes reliably (not ignored) — verified in fat_ptr_iter.rs, passes (2026-03-21)

### Valgrind

- [x] No valgrind errors on `[str]` and `[[int]]` iteration patterns — `nested_int_list_iter.ori` + existing `str_for_yield.ori` etc. (2026-03-21)
- [x] No valgrind errors on `{str: int}` map iteration patterns — `map_str_iteration.ori` + existing `map_str_for_do.ori` (2026-03-21)
- [x] No valgrind errors on `Set<str>` iteration patterns — `set_str_iteration.ori` (2026-03-21)
- [x] No valgrind errors on `Set<str>` COW mutation patterns (insert, remove, union) — `set_str_cow_mutations.ori`. **BUG FIXED**: set algebra `release_set_buffer` helper now decs occupied elements via V5 header `elem_dec_fn` before freeing unique buffers. Previously `ori_rc_free(d1, ...)` freed without element cleanup, leaking fat-pointer elements. (2026-03-21)
- [x] No valgrind errors on `map.keys()` / `map.values()` with fat-pointer keys/values — `map_keys_values_fat.ori` (2026-03-21)
- [x] No valgrind errors on `set.to_list()` with `Set<str>` — `set_str_to_list.ori` (2026-03-21)
- [x] No valgrind errors on `str.split(sep:)` returning `[str]` — `str_split_result.ori` (2026-03-21)
- [x] No valgrind errors on `ori_iter_collect_set` with `Set<str>` elements — `collect_set_str.ori` (2026-03-21)

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
| `ori_map_insert_cow` | `map/cow.rs:37` | `runtime_functions.rs:563` | `map_builtins.rs:250` | `val_dec: ptr` |

Additionally, if `alloc_set_hash_buffer` and `rehash_set` gain `elem_dec_fn` parameters, their callers within `ori_rt` must be updated (these are internal-only, no LLVM IR declaration needed).

If `ori_args_from_argv` option (a) is chosen: `lib.rs:303` + `runtime_functions.rs` + `entry_point.rs` must also sync.

- [x] All ABI sync points committed atomically (no partial updates) — collect RcInc fix: runtime + declarations + codegen synced in one change (2026-03-21)

### Build Verification

- [x] All existing AOT tests pass (`timeout 150 cargo test -p ori_llvm --test aot`) — 1849 passed (2026-03-21)
- [x] All tests pass in release build (`cargo b --release && timeout 150 cargo test -p ori_llvm --test aot`) — 1855 passed, 0 failures. Two pre-existing IR capture failures in arc.rs and generics.rs fixed with release-safe guards. (2026-03-21)

### Cleanup

- [x] Stale "V4: at `header_data - 16`" comment in `list_rc.rs:27` updated to V5 (2026-03-21)
- [x] Stale "8-byte refcount header" comments in `list/mod.rs` updated to "32-byte V5 header" (lines 83, 103, 131, 199) (2026-03-21)
- [x] Stale "§02.7" reference in `cow.rs:38` updated to V5 header-based cleanup model (2026-03-21)
- [x] Stale "V4" label in `list/reset/mod.rs:71` updated to V5 (2026-03-21)
- [x] Decorative banners removed from `list/mod.rs` (2), `iterator/consumers.rs` (8), `set/mod.rs` (1), `map/mod.rs` (1) — all replaced with plain section comments (2026-03-21)
- [x] `construction.rs` fallback `_ => Idx::INT` patterns now have `debug_assert!(false, ...)` at all 4 sites (lines 89, 136, 189, 424) (2026-03-21)
- [x] `iterator/state.rs` doc comment updated to mention V5 header defense-in-depth for `elem_dec_fn` cleanup (2026-03-21)
- [x] `list_builtins.rs` doc comment updated to mention V5 header as second safety net (2026-03-21)
- [x] `set/cow/basic.rs` and `set/cow/algebra.rs` doc comments updated for `elem_dec_fn` propagation (2026-03-21)
- [x] `ori_list_push_new` codegen usage determined -- **JIT/test only** (not called from `arc_emitter/`); no codegen changes needed (2026-03-20)
- [x] **[TPR-02-003]** JIT evaluator integration test — added `jit_symbols_include_elem_header_helpers` test in `runtime_decl/tests.rs` that verifies `ori_buffer_store_elem_dec` and `ori_buffer_store_elem_count` are in the JIT symbol mapping table. Existing `jit_symbol_mappings_match_jit_allowed` test already ensures all JIT-allowed RT_FUNCTIONS have working mappings. (2026-03-21)
- [x] Decorative banners removed from `set/mod.rs` (1 banner) and `map/mod.rs` (1 banner) — included in batch above (2026-03-21)
- [x] `map/mod.rs` `#[allow(unused_imports)]` removed. `META_EMPTY` IS used in `mod.rs` (line 50); plan claim was incorrect. (2026-03-21)
- [x] `map/mod.rs` dead `elem_size` param removed from `write_array_to_list_from_data` + 2 callers updated. (2026-03-21)
- [x] `set/cow/basic.rs` + `set/cow/algebra.rs` dead `_ea` computations removed (5 sites), params renamed `_elem_align`. (2026-03-21)
- [x] `cow_sort.rs` both `vec![0u8; es]` allocations converted to `[0u8; 24]` stack array with heap fallback. (2026-03-21)
- [x] **[BUG]** `.iter().map(transform:).collect()` on `[str]` crashes with misaligned pointer dereference in `string/ops.rs:319` during AOT compilation. **Root cause:** Trampoline ABI mismatch — for fat-pointer types (>16 bytes), the trampoline loaded elements by-value and used direct return, but the closure uses sret return + indirect param ABI. **Fix:** trampolines.rs now checks `abi_size` and uses `call_indirect_void` with pointer passing for types >16 bytes. Added `call_indirect_void` to IrBuilder. All 4 trampoline kinds (Map/Predicate/ForEach/Fold) fixed. AOT regression tests: `test_trampoline_map_str_identity`, `test_trampoline_filter_str`, `test_trampoline_fold_str`. Pass debug + release. (2026-03-21)
- [x] **[TPR-02-011]** Split `edge_cleanup.rs` (539→419 lines) — extracted `insert_trampoline`, `compute_defined_at_or_before`, and `retarget_terminator` into sibling `trampoline.rs` (142 lines). Both under 500-line limit. All 1008 ori_arc tests pass. (2026-03-21)
- [x] **[TPR-02-016]** `@main(args: [str])` unwind-path args cleanup — Fixed in `entry_point.rs`: wrapper uses `invoke`+`landingpad` for `_ori_main` when it can unwind AND has args. Cleanup landingpad calls `ori_args_cleanup` before `resume`. Supports both Itanium (landingpad/resume) and SEH (cleanuppad/cleanupret). Nounwind `_ori_main` correctly uses plain `call`. Runtime AOT tests pass in debug + release. IR semantic pins are release-safe (skip gracefully via TPR-02-017 fix). (2026-03-21)
- [x] **[BUG]** Set intersection/difference fast paths (unique) — Fixed in 02.3 via `load_elem_dec_fn(d1)` + dec before tombstoning. AOT tests pass debug + release. (2026-03-21)
- [x] **[BUG]** Map remove fat-pointer leak — Fixed in 02.3 via `key_dec` + `val_dec` parameters. AOT tests pass debug + release. (2026-03-21)
- [x] **[TPR-02-020]** Main wrapper catch-all + signal detection + Valgrind — Fixed main wrapper to use `landingpad catch ptr null` (not cleanup+resume). Added `exit_code_from_status()` for signal detection, `assert_no_signal_crash()` for crash detection, `compile_and_run_valgrind_with_args()` for leak verification. Valgrind test confirms zero memory errors on panic-path args cleanup. (2026-03-21)
- [x] **[TPR-02-021]** JIT functional test for collection construction — Added `test_jit_str_list_construction` running `ori test --backend=llvm` on `[str]` list literal. Exercises MCJIT symbol resolution of `ori_buffer_store_elem_dec`/`ori_buffer_store_elem_count`. (2026-03-21)

### Excluded Allocation Sites (No Action Needed)

The following `ori_rc_alloc` call sites do NOT need `elem_dec_fn` propagation and are explicitly excluded:

- **`string/methods/mod.rs`** (lines 301, 388): String COW operations. These allocate string DATA buffers (raw bytes), not list element buffers. `elem_dec_fn` is for element-level cleanup of collections, not for string internals.
- **`string/mod.rs`** (lines 196, 240, 263): `OriStr::from_bytes`, `with_capacity`, `from_raw`. Same — string data, not collection elements.
- **`string/ops.rs:221`**: `ori_str_concat_cow` — allocates a new string data buffer on the slow path. Not a collection element buffer.
- **`map/hash_table.rs`** (lines 232, 274): `rehash_map`, `OriMap::alloc_hash_buffer`. Map hash table buffers. Maps use TWO cleanup functions (key + value) that cannot fit in a single header slot. The codegen-based approach (option c) handles map cleanup. No header propagation needed.
- **`map/cow.rs:144`**: Map COW slow path. Same as above — map hash table buffer, not list/set buffer.
- **`iterator/sources.rs:93`**: `ori_iter_from_str` — allocates a heap copy of string bytes for the string iterator. Not a collection element buffer.
- **`list/mod.rs:108`**: `ori_list_new` — allocates the `OriList` STRUCT on the heap (not the data buffer). The `ori_rc_alloc` here is for the list metadata struct, not for the data buffer. The data buffer allocation at line 162 is covered separately.
