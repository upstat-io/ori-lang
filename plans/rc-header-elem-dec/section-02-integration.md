---
section: "02"
title: "Codegen & Runtime Integration"
status: not-started
goal: "Wire up LLVM codegen and runtime so elem_dec_fn is stored at list creation time, and iterator Drop reads it from the header"
depends_on: ["01"]
reviewed: false
third_party_review:
  status: none
  updated: null
sections:
  - id: "02.1"
    title: "Store elem_dec_fn at List Construction"
    status: not-started
  - id: "02.2"
    title: "Iterator Creation and Drop"
    status: not-started
  - id: "02.3"
    title: "Map and Set Integration"
    status: not-started
  - id: "02.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "02.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 02: Codegen & Runtime Integration

**Status:** Not Started
**Goal:** With the RC header now storing `elem_dec_fn`, wire up the LLVM codegen to store the function at list creation time, and update the iterator runtime to read it from the header.

**Depends on:** Section 01 (RC header must be extended first).

---

## 02.1 Store elem_dec_fn at List Construction

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/construction.rs`, `compiler/ori_rt/src/list/mod.rs`
<!-- reviewed: accuracy fix — list construction is in construction.rs (CtorKind::ListLiteral), not list_builtins.rs -->

When a list literal `[a, b, c]` is constructed, the codegen's `emit_construct` (via `CtorKind::ListLiteral`) allocates a buffer via `ori_list_alloc_data`. After storing elements, it must also store the `elem_dec_fn` in the buffer's RC header.

- [ ] Add runtime function `ori_buffer_store_elem_dec(data: *mut u8, elem_dec_fn: Option<extern "C" fn(*mut u8)>)` in `compiler/ori_rt/src/rc/mod.rs` (or a new `elem_dec.rs` submodule) — wrapper around `store_elem_dec_fn` that's `#[no_mangle] extern "C"` callable from LLVM IR
- [ ] Declare `ori_buffer_store_elem_dec` in `compiler/ori_llvm/src/codegen/runtime_decl/runtime_functions.rs` — signature: `(ptr, ptr) -> void` (data pointer + function pointer) <!-- reviewed: completeness fix — LLVM-side declaration is required for any new runtime function -->
- [ ] In the `Construct` handler for lists (in `construction.rs`): after storing all elements in the buffer, emit a call to `ori_buffer_store_elem_dec(data_ptr, elem_dec_fn)` where `elem_dec_fn` is `get_or_generate_elem_dec_fn(element_type)`. Note: there is no `emit_list_literal` function — list construction goes through the `Construct` ARC instruction handler. <!-- reviewed: accuracy fix — emit_list_literal does not exist; lists are constructed via Construct instruction -->
- [ ] For lists with scalar elements (int, float, bool, etc.): `elem_dec_fn` is NULL — still call `ori_buffer_store_elem_dec(data, NULL)` for consistency (no-op since header is zero-initialized)
- [ ] For `ori_list_alloc_data`: no change needed — it allocates via `ori_rc_alloc` which zero-initializes the elem_dec_fn slot
- [ ] **Set construction**: Set construction also goes through `Construct` in `construction.rs`. Apply the same `ori_buffer_store_elem_dec` call after set buffer allocation. Sets use `ori_set_buffer_rc_dec` which takes `elem_dec_fn` — the header approach applies identically. <!-- reviewed: completeness fix — sets were not mentioned in construction -->
- [ ] **COW operations that reallocate**: List COW mutations (`push`, `insert`, `set`, etc.) may call `ori_rc_realloc` which preserves all header bytes. Verify that `ori_rc_realloc` copies the `elem_dec_fn` field during reallocation (it does, since `realloc` preserves `min(old, new)` bytes and the header is at the front). Add a unit test for this. <!-- reviewed: completeness fix — realloc interaction with elem_dec_fn -->
- [ ] **`ori_list_reset_buffer` interaction**: The list reset function in `compiler/ori_rt/src/list/reset/mod.rs` allocates a new buffer when the old one can't be reused. The new buffer's `elem_dec_fn` must be set. Currently, `ori_list_reset_buffer` calls `ori_rc_alloc` (which zero-initializes) — the codegen must call `ori_buffer_store_elem_dec` on the returned buffer. Verify the construction.rs code path for list reuse (`CtorKind::ListLiteral` with reuse token, lines ~395-460) stores elem_dec_fn on the new/reused buffer. <!-- reviewed: completeness fix — list reset/reuse path -->
- [ ] **COW slow path interaction**: `ori_list_push_cow` (and `_insert_cow`, `_set_cow`, `_remove_cow`, `_reverse_cow` in `compiler/ori_rt/src/list/cow.rs`) allocate new buffers via `ori_rc_alloc` on the slow path (shared owner or empty). The new buffer's `elem_dec_fn` is zero-initialized. The `elem_dec_fn` must be propagated from the old buffer to the new buffer. Two approaches: (a) read `elem_dec_fn` from the old buffer's header and write it to the new buffer's header inside the runtime COW function, or (b) rely on the next `ori_buffer_rc_dec` call to write it via the store-then-read pattern. Option (b) works if the codegen always passes the real `elem_dec_fn` to `ori_buffer_rc_dec` for the new buffer's type — verify this is guaranteed. Option (a) is safer and requires ~3 lines per COW function. **Recommended**: Option (a) — read from old header, write to new header in every COW slow path that creates a new buffer. <!-- reviewed: completeness fix — CRITICAL: COW slow path creates buffers without elem_dec_fn -->
- [ ] Add AOT test: construct `[str]`, verify `ORI_CHECK_LEAKS=1` shows zero leaks after the list goes out of scope (without iteration)
- [ ] Add AOT test: construct `[[int]]`, verify zero leaks after the outer list goes out of scope
- [ ] Add AOT test: `[str]` COW push (shared list, push creates new buffer) — verify the new buffer has correct `elem_dec_fn` and elements are cleaned up <!-- reviewed: completeness fix -->

### Cleanup <!-- reviewed: hygiene fix -->

- [ ] **[WASTE]** `compiler/ori_llvm/src/codegen/arc_emitter/construction.rs:89` — The fallback `_ => ori_types::Idx::INT` in the `ListLiteral` match arm silently returns INT as element type when TypeInfo doesn't match List. This masks bugs. Add a `tracing::warn!` or `debug_assert!` so misclassification is visible.
- [ ] **[WASTE]** `compiler/ori_llvm/src/codegen/arc_emitter/construction.rs:137` — `let _ = elem_ty;` suppress warning inside `emit_list_iter` comment block — this dead-code marker is in `list_builtins.rs:137`, not construction.rs. Verify and remove after `elem_dec_fn` is no longer NULL (it should be used for `get_or_generate_elem_dec_fn` or removed).

---

## 02.2 Iterator Creation and Drop

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/builtins/collections/list_builtins.rs`, `compiler/ori_rt/src/iterator/state.rs`, `compiler/ori_rt/src/iterator/sources.rs`

With `elem_dec_fn` stored in the header at construction time, the iterator no longer needs it as a parameter. The runtime reads it from the header when RC reaches zero.

- [ ] `emit_list_iter`: Continue passing NULL for `elem_dec_fn` to `ori_iter_from_list` — the runtime will read from the header. (Or remove the `elem_dec_fn` parameter entirely from the runtime function signature — see decision below.)
- [ ] **Decision**: Keep the `elem_dec_fn` parameter in `ori_iter_from_list` for backward compatibility? Or remove it?
  - **Option A (simpler)**: Keep parameter, always pass NULL from codegen. Runtime ignores it, reads from header. Minimal ABI change.
  - **Option B (cleaner)**: Remove `elem_dec_fn` parameter from `ori_iter_from_list` and `IterState::List`. Runtime always reads from header. Requires updating all call sites.
  - Implement **Option A** first. Option B cleanup is mandatory and must be done in Section 03 (remove workarounds) — it is NOT optional. The dead parameter is tech debt that must be removed in the same plan. <!-- reviewed: completeness fix — eliminated deferral trap; "if time permits" is banned -->
- [ ] `IterState::List` Drop impl: Change `ori_buffer_rc_dec(data, len, cap, elem_size, elem_dec_fn)` to `ori_buffer_rc_dec(data, len, cap, elem_size, NULL)` — the runtime reads elem_dec_fn from the header. (This is already the case if passing NULL above.)
- [ ] Verify: when iterator's `ori_buffer_rc_dec` reaches zero, it reads elem_dec_fn from header and performs element cleanup
- [ ] Verify: when explicit RcDec's `ori_buffer_rc_dec` reaches zero, same behavior — reads from header
- [ ] Test: `[str]` iteration where iterator dec reaches zero first — elements cleaned via header function
- [ ] Test: `[str]` iteration where explicit dec reaches zero first — same behavior
- [ ] Test: function parameter `[str]` — callee iterates, caller uses after — no double-free, no leak

### Cleanup <!-- reviewed: hygiene fix -->

- [ ] **[WASTE]** `compiler/ori_llvm/src/codegen/arc_emitter/builtins/collections/list_builtins.rs:112-120` — Large doc comment block on `emit_list_iter` explaining the phantom `__for_coll` workaround and NULL `elem_dec_fn` rationale will be stale after this plan. Rewrite to document the header-based approach.
- [ ] **[WASTE]** `compiler/ori_llvm/src/codegen/arc_emitter/builtins/collections/list_builtins.rs:134-138` — The `let _ = elem_ty;` dead code marker and the comment "elem_ty used only for elem_size above" become wrong when `elem_dec_fn` is removed or populated. Remove marker, use `elem_ty` for `get_or_generate_elem_dec_fn` or remove entirely.

---

## 02.3 Map and Set Integration

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/rc_buffer_ops.rs`, `compiler/ori_rt/src/map/mod.rs`

Maps and sets also have element cleanup needs. Extend the header-based approach.

- [ ] Maps: The map buffer stores keys and values. Currently `ori_map_buffer_rc_dec` takes `key_dec_fn` and `val_dec_fn` separately. With the header approach, we need TWO slots or a combined approach.
  - **IMPORTANT**: `IterState::Map` Drop DOES call `ori_map_buffer_rc_dec` when `owns_data` is true (see `compiler/ori_rt/src/iterator/state.rs` lines 171-197). This means maps have the SAME ordering issue as lists — the map iterator's Drop passes whatever `key_dec_fn`/`val_dec_fn` were stored at construction time. <!-- reviewed: accuracy fix — map iterator Drop DOES call ori_map_buffer_rc_dec; the original claim that it doesn't was wrong -->
  - **Verified**: `emit_map_iter` in `map_builtins.rs` (line ~340-344) passes NULL for both `key_dec_fn` and `val_dec_fn`, just like `emit_list_iter` passes NULL for `elem_dec_fn`. Maps therefore have the same ordering-dependent cleanup issue as lists. <!-- reviewed: accuracy fix — verified emit_map_iter passes NULL -->
  - **IMPORTANT**: The `__for_coll` phantom binding in `loops.rs` line 174 only matches `List | Set`, NOT `Map`. This means maps have NO ordering workaround at all today. Maps are either (a) silently leaking/double-freeing with str keys, or (b) not yet failing because `test_map_str_key_iteration` happens to get lucky ordering. This must be investigated and fixed as part of this section. <!-- reviewed: completeness fix — maps lack the existing workaround too -->
  - **Recommended approach**: Change `emit_map_iter` to pass real `key_dec_fn`/`val_dec_fn` instead of NULL. This is simpler than extending the header for maps and avoids the two-slot problem. The `IterState::Map` already stores both functions — just populate them with real values. Combined with the header approach for lists/sets, this ensures all collection types have correct cleanup. <!-- reviewed: completeness fix — concrete recommendation -->
- [ ] Sets: Same as lists — the set buffer stores elements, `ori_set_buffer_rc_dec` takes `elem_dec_fn`. Apply the same header-store pattern.
- [ ] **Set iteration codegen**: Verify how `emit_list_iter` is called for sets (it's dispatched at `builtins/collections/mod.rs` line 465 as `("Set", "iter") => emit_list_iter`). Since `emit_list_iter` passes NULL for `elem_dec_fn`, sets have the same issue as lists. The header approach resolves this automatically — sets share the same buffer layout and `ori_buffer_rc_dec` path as lists (just with hash table metadata). <!-- reviewed: completeness fix — set iteration shares list path -->
- [ ] Verify: map iteration with str keys passes with zero leaks (currently passing `test_map_str_key_iteration` — but may be order-dependent; run 10x to confirm stability)
- [ ] Verify: set iteration with str elements passes with zero leaks
- [ ] Add AOT test: map with str keys, passed to function, iterated inside — same pattern as `test_str_list_passed_to_two_functions` but for maps <!-- reviewed: completeness fix — map equivalent of the motivating test -->

### Cleanup <!-- reviewed: hygiene fix -->

- [ ] **[WASTE]** `compiler/ori_llvm/src/codegen/arc_emitter/builtins/collections/map_builtins.rs:325-327` — Doc comment on `emit_map_iter` states "Null `elem_dec` functions prevent double-free on maps with RC-managed keys/values." After this fix, update the doc to explain the correct approach (either header-based or codegen-passing-real-functions).
- [ ] **[WASTE]** `compiler/ori_llvm/src/codegen/arc_emitter/builtins/collections/map_builtins.rs:342` — `let _ = (key_ty, val_ty);` dead code marker that suppresses the unused-variable warning. After this plan passes real dec functions, this marker becomes incorrect. Remove it when the real `key_dec_fn`/`val_dec_fn` are passed.

---

## 02.R Third Party Review Findings

- None.

---

## 02.N Completion Checklist

- [ ] `ori_buffer_store_elem_dec` runtime function exists and is callable from LLVM IR
- [ ] List construction stores `elem_dec_fn` in RC header
- [ ] Set construction stores `elem_dec_fn` in RC header <!-- reviewed: completeness fix -->
- [ ] Map iteration passes real `key_dec_fn`/`val_dec_fn` (not NULL) to `ori_iter_from_map` <!-- reviewed: completeness fix -->
- [ ] Iterator creation and drop use NULL for `elem_dec_fn` parameter (header provides it)
- [ ] `ori_buffer_store_elem_dec` declared in `runtime_functions.rs` with correct signature <!-- reviewed: completeness fix — LLVM declaration sync -->
- [ ] `test_str_list_passed_to_two_functions` passes (unignore and verify)
- [ ] `test_nested_list_iteration` passes (unignore and verify)
- [ ] All existing AOT tests pass (`timeout 150 cargo test -p ori_llvm --test aot`) <!-- reviewed: completeness fix — added timeout -->
- [ ] No valgrind errors on `[str]` and `[[int]]` iteration patterns
- [ ] No valgrind errors on `{str: int}` map iteration patterns <!-- reviewed: completeness fix -->
