---
section: "02"
title: "Codegen & Runtime Integration"
status: in-progress
goal: "Wire up LLVM codegen and runtime so elem_dec_fn and elem_count are stored in the RC header at collection construction time, and all buffer-freeing paths read from the header"
depends_on: ["01"]
reviewed: false
third_party_review:
  status: resolved
  updated: 2026-03-21
sections:
  - id: "02.1"
    title: "Store elem_dec_fn and elem_count at Collection Construction"
    status: in-progress
  - id: "02.2"
    title: "Iterator Creation and Drop"
    status: complete
  - id: "02.3"
    title: "Map and Set Integration"
    status: in-progress
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
- [ ] **[WASTE]** `ori_list_push_new` is declared in `runtime_functions.rs` but has no codegen callers — remove the declaration from `runtime_functions.rs` and the JIT symbol mapping from `runtime_mappings.rs`. Tracked in Section 03.2.5 for cleanup alongside `ori_iter_from_list` parameter removal.

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
- [ ] `map.keys()` on `{str: int}` -- double-free (map standalone RcDec + map_buffer_cleanup both fire). AOT test `test_map_keys_str_scope_drop` written and `#[ignore]`. Tracked in 02.N map double-free item.
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
- [ ] **[WASTE]** `cow_sort.rs:256` -- `ori_list_reverse_cow` fast path allocates `vec![0u8; es]` on every call. For common element sizes (8, 16, 24 bytes), use a stack array `[0u8; 24]` with heap fallback for larger sizes.
- [ ] **[WASTE]** `cow_sort.rs:458` -- `apply_permutation_in_place` allocates `vec![0u8; elem_size]` similarly. Same fix: stack array `[0u8; 24]` with heap fallback. Address alongside line 256 since both are in the same file.
- [ ] **[LATENT]** `query.rs` lines 142, 199 -- `ori_list_reverse` and `ori_list_concat` use hardcoded alignment `8` in `ori_rc_alloc`. Safe for current Ori types (max align 8) but incorrect if element alignment exceeds 8. Add `elem_align` parameter or use a centralized alignment lookup.
- [ ] **[WASTE]** `map/mod.rs:21-25` -- `#[allow(unused_imports, reason = "used by cow.rs after rewrite")]` masks actual unused import: `META_EMPTY` is NOT used by `cow.rs` (only `META_OCCUPIED` and `META_TOMBSTONE` are). Either remove `META_EMPTY` from the re-export or change to `#[expect(unused_imports)]` so the lint fires when the situation changes.
- [ ] **[WASTE]** `map/mod.rs:329` -- `let _ = elem_size;` in `write_array_to_list_from_data` explicitly discards the `elem_size` parameter. The function accepts it but never uses it (callers already sized the buffer externally). Either remove the parameter (and update 2 callers at lines 133, 179) or use it for a `debug_assert!` on buffer size.
- [ ] **[WASTE]** `set/cow/basic.rs` lines 38, 147 and `set/cow/algebra.rs` lines 80, 192, 312 -- `let _ea = elem_align.max(1) as usize;` computes alignment then discards it with underscore prefix. Five occurrences across two files. The `elem_align` parameter is accepted from codegen but never used (all `alloc_set_hash_buffer` and `rehash_set` calls hardcode alignment `8`). Either use `_ea` in the allocation calls or remove the dead computation.
- [x] **[STYLE]** `set/mod.rs:98` -- decorative banner replaced with plain section comment (2026-03-21)
- [x] **[STYLE]** `map/mod.rs:228` -- decorative banner replaced with plain section comment (2026-03-21)
- [ ] **[BLOAT]** `construction.rs` is 499 lines -- at the limit. If any further work is needed in this file, extract `emit_variant_via_alloca` and `emit_variant_via_insertvalue` (total ~130 lines) into a `variant_construction.rs` submodule before exceeding 500 lines.

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
- [ ] **[BUG]** `cow_insert_existing` fast path (unique) leaks the old value when overwriting — `copy_nonoverlapping` overwrites the old value in the buffer without calling `val_dec_fn` first. Requires adding `val_dec` parameter to `cow_insert_existing` (and `ori_map_insert_cow`), plus codegen changes to pass the val_dec thunk. Low priority: only triggers when overwriting an existing key in a unique map with fat-pointer values.

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
- [ ] AOT test: `Set<str>` collect with heap strings (>23 bytes) — zero leaks via `ORI_CHECK_LEAKS=1` in debug + release (blocked on `.iter().map().collect()` crash — see 02.N)

### AOT Tests

- [x] `{str: int}` map iteration -- zero leaks via `ORI_CHECK_LEAKS=1` (10x stability check passed). AOT test `test_map_str_iteration` added to `elem_dec_scope.rs`. (2026-03-21)
- [ ] `Set<str>` iteration -- verify zero leaks
- [x] `{str: int}` map passed to function, iterated inside -- zero leaks. AOT test `test_map_str_passed_to_fn` added to `elem_dec_scope.rs`. (2026-03-21)
- [ ] `Set<str>` passed to function, iterated inside -- verify both header-based and parameter-based cleanup paths work, zero leaks
- [ ] `Set<str>` COW insert on shared set -- verify new buffer has correct `elem_dec_fn` and zero leaks
- [ ] `Set<str>` union/intersection/difference -- verify new buffer cleanup, zero leaks
- [x] `map.keys()` on `{str: int}` -- zero leaks. `test_map_keys_str_scope_drop` AOT test passes in debug + release. Exercises `ori_map_keys_to_list` with `key_inc_fn`. (2026-03-21)

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
- [ ] `Set<str>` iteration -- zero leaks
- [x] `{str: int}` map passed to function, iterated inside -- zero leaks (`test_map_str_passed_to_fn` AOT test) (2026-03-21)
- [ ] `Set<str>` passed to function, iterated inside -- zero leaks
- [ ] `Set<str>` COW insert on shared set -- zero leaks
- [ ] `Set<str>` union/intersection/difference -- zero leaks
- [ ] `set.to_list()` on `Set<str>` -- exercises `ori_set_to_list` creating a new `[str]` buffer, zero leaks
- [ ] `ori_iter_collect_set` on `Set<str>` via `for x in items yield x` with set target -- output set buffer has correct `elem_dec_fn`, zero leaks
- [ ] **[TPR-02-005]** Fix `generate_main_wrapper` in `entry_point.rs` — wrapper loads sret result as `{i64, i64, ptr}` struct value but `_ori_main` expects `ptr` (Indirect ABI for 24-byte param). Must consult callee's `param_abi.passing` and pass pointer for Indirect params instead of loading the value. Blocks `@main(args: [str])` AOT test below.
- [ ] `@main(args: [str])` with arguments -- exercises `ori_args_from_argv` creating `[str]` buffer, zero leaks (run AOT binary with args, verify `ORI_CHECK_LEAKS=1` clean). Blocked on TPR-02-005 fix above.
- [x] `test_str_list_passed_to_two_functions` passes reliably (not ignored) — verified 81/81 fat_ptr_iter tests pass (2026-03-21)
- [x] `test_nested_list_iteration` passes reliably (not ignored) — verified in fat_ptr_iter.rs, passes (2026-03-21)

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

- [x] All ABI sync points committed atomically (no partial updates) — collect RcInc fix: runtime + declarations + codegen synced in one change (2026-03-21)

### Build Verification

- [ ] All existing AOT tests pass (`timeout 150 cargo test -p ori_llvm --test aot`)
- [ ] All tests pass in release build (`cargo b --release && timeout 150 cargo test -p ori_llvm --test aot`)

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
- [ ] **[TPR-02-003]** Add functional JIT evaluator integration test that compiles a list/set literal through MCJIT and executes successfully (regression guard for JIT symbol availability)
- [x] Decorative banners removed from `set/mod.rs` (1 banner) and `map/mod.rs` (1 banner) — included in batch above (2026-03-21)
- [ ] `map/mod.rs:21-25` `#[allow(unused_imports)]` cleaned up: remove `META_EMPTY` from re-export (unused by `cow.rs`)
- [ ] `map/mod.rs:329` dead `let _ = elem_size;` in `write_array_to_list_from_data` resolved (remove parameter or add assertion)
- [ ] `set/cow/basic.rs` + `set/cow/algebra.rs` dead `_ea` computations removed or used (5 sites)
- [ ] `cow_sort.rs:458` `vec![0u8; elem_size]` in `apply_permutation_in_place` converted to stack array with heap fallback
- [ ] **[BUG]** `.iter().map(transform:).collect()` on `[str]` crashes with misaligned pointer dereference in `string/ops.rs:319` during AOT compilation. Pre-existing bug in closure trampoline + iterator adapter chain codegen. Discovered during collect RcInc fix testing.
- [ ] **[TPR-02-011]** Split `edge_cleanup.rs` (529 lines, over 500-line limit) — extract merge-edge filtering helpers into a sibling module (e.g., `merge_edge_cleanup.rs`) before adding more RC edge logic.

### Excluded Allocation Sites (No Action Needed)

The following `ori_rc_alloc` call sites do NOT need `elem_dec_fn` propagation and are explicitly excluded:

- **`string/methods/mod.rs`** (lines 301, 388): String COW operations. These allocate string DATA buffers (raw bytes), not list element buffers. `elem_dec_fn` is for element-level cleanup of collections, not for string internals.
- **`string/mod.rs`** (lines 196, 240, 263): `OriStr::from_bytes`, `with_capacity`, `from_raw`. Same — string data, not collection elements.
- **`string/ops.rs:221`**: `ori_str_concat_cow` — allocates a new string data buffer on the slow path. Not a collection element buffer.
- **`map/hash_table.rs`** (lines 232, 274): `rehash_map`, `OriMap::alloc_hash_buffer`. Map hash table buffers. Maps use TWO cleanup functions (key + value) that cannot fit in a single header slot. The codegen-based approach (option c) handles map cleanup. No header propagation needed.
- **`map/cow.rs:144`**: Map COW slow path. Same as above — map hash table buffer, not list/set buffer.
- **`iterator/sources.rs:93`**: `ori_iter_from_str` — allocates a heap copy of string bytes for the string iterator. Not a collection element buffer.
- **`list/mod.rs:108`**: `ori_list_new` — allocates the `OriList` STRUCT on the heap (not the data buffer). The `ori_rc_alloc` here is for the list metadata struct, not for the data buffer. The data buffer allocation at line 162 is covered separately.
