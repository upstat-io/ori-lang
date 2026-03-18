---
section: "01"
title: "Root Cause Analysis & Design"
status: complete
goal: "Document the full causal chain from codegen through runtime that produces leaked elements and double-frees in iterator-based loops, establishing the shared understanding needed for Sections 02-06"
third_party_review:
  status: none
  updated: null
sections:
  - id: "01.1"
    title: "NULL elem_dec_fn Bug Chain"
    status: complete
  - id: "01.2"
    title: "For-Yield Spurious RcDec Bug Chain"
    status: complete
---

# Section 01: Root Cause Analysis & Design

**Status:** Not Started
**Goal:** Fully document the two interacting bugs -- NULL `elem_dec_fn` in `emit_list_iter()` and the spurious extra `RcDec` in for-yield lowering -- tracing each from root cause through the pipeline to the observable failure. No code changes in this section; it establishes the analysis that drives Sections 02-06.

**Context:** The iterator-collection RC ownership contract has two bugs that interact destructively. Bug 1 (NULL `elem_dec_fn`) means that whichever dec reaches zero on the list buffer will fail to clean up elements with RC children (str, nested lists, closures, etc.), causing memory leaks. Bug 2 (for-yield spurious RcDec) means the AIMS pipeline emits 3 decs for 2 incs on the source collection, causing a double-free. Together, they mean for-yield on `[str]` or `[Option<str>]` both leaks elements AND double-frees the buffer.

---

## 01.1 NULL elem_dec_fn Bug Chain

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/builtins/collections/list_builtins.rs` (emit_list_iter, line 115-140), `compiler/ori_rt/src/iterator/sources.rs` (ori_iter_from_list), `compiler/ori_rt/src/iterator/state.rs` (IterState::Drop), `compiler/ori_rt/src/rc/list_rc.rs` (ori_buffer_rc_dec, drop_elements_and_free)

The bug chain from codegen to observable leak:

1. **Codegen origin** (`list_builtins.rs:115-140`): `emit_list_iter()` is called when lowering `list.iter()`. It calls `ori_iter_from_list(data, len, cap, elem_size, elem_dec_fn)`. Previously, `elem_dec_fn` was hardcoded to `const_null(ptr_type)` -- a NULL function pointer.

2. **Runtime storage** (`sources.rs:27-43`): `ori_iter_from_list()` stores the `elem_dec_fn` parameter into `IterState::List { ..., elem_dec_fn }`. With the NULL from codegen, this field is `None`.

3. **Iterator drop** (`state.rs:127-155`): When the iterator is dropped (either by `ori_iter_drop` or Rust's automatic Drop), `IterState::List::drop()` calls `ori_buffer_rc_dec(data, len, cap, elem_size, elem_dec_fn)` (guarded by `!data.is_null() && *cap != 0`). With NULL `elem_dec_fn`, this call cannot clean up element-level RC.

4. **Buffer cleanup** (`list_rc.rs:24-38`): `drop_elements_and_free()` checks `if let Some(f) = elem_dec_fn`. When `elem_dec_fn` is `None`, the element cleanup loop is skipped entirely. The buffer memory is freed via `ori_rc_free`, but any RC children of elements (e.g., the heap data pointer inside each `str` element of a `[str]`) are never decremented.

5. **Observable failure**: For `[str]`, each string's data buffer leaks. For `[[int]]`, each inner list's buffer leaks. For `[Option<str>]`, the `str` payloads inside `Some` variants leak.

**The __for_coll phantom mechanism** works around this in for-do loops: by threading the collection variable through the loop header as a mutable binding, the AIMS backward analysis sees a "future use" after `ori_iter_drop` and schedules the collection's explicit `RcDec` (which carries the real `elem_dec_fn` from codegen) as the LAST dec. Since the explicit dec reaches zero first, elements are cleaned up correctly despite the iterator's NULL `elem_dec_fn`.

**Why the workaround is fragile**: The design principle "the AIMS dec always reaches zero" is an ordering assumption, not an invariant. Any code path where `ori_iter_drop`'s internal dec reaches zero before the AIMS-emitted explicit dec will fail silently. For-yield is one such path. Others include: early iterator drop via `drop_early()`, iterator adapters that consume the source iterator, and cross-function iterator passing.

**Design principle established**: ANY dec may be the final dec. The `elem_dec_fn` must be correct everywhere, not just on the "expected" final dec path. The fix (Section 02) is to pass the real `elem_dec_fn` from `get_or_generate_elem_dec_fn(elem_ty)` in `emit_list_iter()`.

**UPDATE (2026-03-18):** The list/set `elem_dec_fn` fix was already applied during fat-pointer-hardening. `emit_list_iter()` at `list_builtins.rs:133` now calls `self.get_or_generate_elem_dec_fn(elem_ty)` instead of `const_null(ptr_type)`. Sets route through `emit_list_iter()` and are also fixed. Only the **map path** (`emit_map_iter()` at `map_builtins.rs:343-344`) still passes NULL for `key_dec_fn`/`val_dec_fn`. Section 02 scope is reduced to map-only.

- [x] Trace the full codegen path: `lower_for()` -> `lower_for_yield()` -> `prepare_iterator()` -> ARC IR `Apply("iter", ...)` -> LLVM `emit_list_iter()` -> `ori_iter_from_list()` call — traced: `for_yield.rs:prepare_iterator()` (lines 56-92) resolves collection type and creates coll_var; `lower_for_yield_iterator()` (lines 208-346) threads collection through loop blocks; AIMS sees `Apply("iter", ...)` → LLVM `emit_list_iter()` at `list_builtins.rs:115-140` → `ori_iter_from_list(data, len, cap, elem_size, elem_dec_fn)` with real elem_dec_fn (line 133) (2026-03-18)
- [x] Trace the runtime drop path: `ori_iter_drop()` -> Rust `Drop for IterState` -> `ori_buffer_rc_dec()` -> `drop_elements_and_free()` — traced: `ori_iter_drop` at `iterator/mod.rs` recasts `*mut u8` to `Box<IterState>` and drops; `IterState::List::Drop` at `state.rs` calls `ori_buffer_rc_dec(data, len, cap, elem_size, elem_dec_fn)` guarded by `!data.is_null() && *cap != 0`; `ori_buffer_rc_dec` at `list_rc.rs:64-110` decrements RC, only calls `drop_elements_and_free` when RC reaches zero; `drop_elements_and_free` at `list_rc.rs:24-38` checks `if let Some(f) = elem_dec_fn` — skips element cleanup if None. With real elem_dec_fn, cleanup runs correctly. (2026-03-18)
- [x] Document which element types are affected (str, [T], closures, structs with Drop fields, Option/Result with fat-pointer payloads) — documented in plan text above and fat-pointer-hardening 01.4 type table (2026-03-18)
- [x] Document which element types are NOT affected (int, float, bool, char, byte, void -- scalar types with no RC children) — documented in plan text above. Scalar types produce `None` from `get_or_generate_elem_dec_fn()` because `DropInfo::is_trivial()` returns true (2026-03-18)
- [x] Verify that `get_or_generate_elem_dec_fn()` in `element_fn_gen.rs` handles all affected element types listed above — verified: generates `_ori_elem_dec$N` functions for each non-trivial DropInfo. Handles FatPointer (str with SSO guard), HeapPointer ([T] recursive), ClosureEnv, AggregateFields (structs), InlineEnum (sum types). Maps use separate key/val dec fns. (2026-03-18)
- [x] Confirm that the for-do `__for_coll` phantom still produces correct results even with the real `elem_dec_fn` — confirmed: `drop_elements_and_free` only runs when `ori_buffer_rc_dec` sees RC reach zero. In for-do, iterator dec (RC=2→1) does NOT trigger cleanup. `__for_coll` phantom dec (RC=1→0) triggers cleanup with real `elem_dec_fn`. No double-cleanup possible because only the dec that reaches zero invokes `drop_elements_and_free`. (2026-03-18)
- [x] Document the parallel map bug: `emit_map_iter()` (`map_builtins.rs:340-344`) passes `const_null_ptr()` for both `key_dec_fn` and `val_dec_fn` — confirmed. Comment at line 340 ("Null elem_dec functions: iterator borrows elements, map's drop function is the single source of truth") is misleading — NULL means element cleanup is skipped if iterator's dec reaches zero first. Maps with str keys/values leak if `ori_iter_drop` is the final dec. No `__for_coll` phantom for maps (phantom only covers `List | Set` at `loops.rs:174`). (2026-03-18)
- [x] Document set coverage: `emit_auto_iter()` routes `TypeInfo::Set` through `emit_list_iter()` at `builtins/mod.rs:371` — confirmed. Both the auto-iter path (line 371: `TypeInfo::List { element } | TypeInfo::Set { element } => emit_list_iter()`) and explicit `.iter()` dispatch (`collections/mod.rs:463-468`: `("Set", "iter") => emit_list_iter()`) use the same `emit_list_iter()`. Sets get the real `elem_dec_fn` automatically. (2026-03-18)
- [x] Document Str iterator path: `emit_str_iter()` at `string_builtins.rs:203-207` calls `ori_iter_from_str(str_ptr)` with no `elem_dec_fn` parameter. `ori_iter_from_str` at `sources.rs:66` creates `IterState::Str { data, len, byte_offset, owns_data }`. Str iterator Drop handles cleanup via `ori_buffer_rc_dec` with `elem_dec_fn=None` when `owns_data` is true — correctly None because char codepoints are scalar (no RC children). Str iteration is NOT affected by the NULL `elem_dec_fn` bug. (2026-03-18)

### Codebase Cleanup (fix alongside analysis)

- [x] **STYLE**: Split merged doc comment in `helpers.rs:177-196` — separated `collect_project_borrowed_defs` doc (moved to line 239) from `collect_iter_element_defs` doc (kept in place). Each function now has its own `///` doc block. (2026-03-18)
- [x] **STYLE**: Add missing `///` doc comment to `collect_project_borrowed_defs` — moved the displaced doc block (previously merged before `collect_iter_element_defs`) to its correct position. (2026-03-18)
- [x] **DOCS**: Update doc comment on `emit_map_iter` — rewritten in Section 02.3: doc now explains real dec fns + `collect_iter_element_defs()` transitive propagation. NULL comment removed. (2026-03-18)
- [x] **TRACKING**: Verify `for_yield.rs:373` TODO reference to `type_strategy_registry/section-11` — confirmed: `plans/type_strategy_registry/section-11-wire-arc-borrow.md` exists. The TODO about extracting shared type layout logic to `ori_ir` is valid and tracked. (2026-03-18)

---

## 01.2 For-Yield Spurious RcDec Bug Chain

**File(s):** `compiler/ori_arc/src/lower/control_flow/for_yield.rs` (prepare_iterator lines 56-92, lower_for_yield_iterator lines 208-346), `compiler/ori_arc/src/lower/control_flow/loops.rs` (__for_coll phantom in for-do lines 174-181), `compiler/ori_arc/src/aims/realize/walk.rs` (emit_defined_dead line 308), `compiler/ori_arc/src/lower/control_flow/for_loops/for_iterator.rs` (exit block dummy reference lines 189-204)

The bug chain from ARC lowering to observable double-free:

1. **For-do collection scoping** (`loops.rs:174-181`): In for-do, the source collection is bound as a mutable variable `__for_coll_N` BEFORE the `.iter()` call (only for `List | Set` tags, not Map -- despite the comment mentioning Map). This binding becomes a mutable scope entry, and the loop lowering (`for_iterator.rs`) threads it through header -> body -> latch -> exit as a block parameter. The exit block emits a dummy `Let { Var(__for_coll_exit_param) }` AFTER `ori_iter_drop` (`for_iterator.rs:196-204`). This creates a clean ordering: the collection's last use is the dummy let, which comes after the iterator drop, so the AIMS backward analysis schedules the collection's RcDec after the iterator's cleanup.

2. **For-yield collection scoping** (`for_yield.rs:56-92`, `for_yield.rs:208-346`): `prepare_iterator()` returns the collection variable as `coll_var: Option<ArcVarId>` (only for `List | Set` tags -- line 85, matching the for-do pattern). `lower_for_yield_iterator()` threads this as a header block param via `coll_param` (lines 250-255) and emits a dummy let in the exit block (lines 337-341). However, the for-yield path differs from for-do in a critical way: the original collection variable `iter_val` remains alive in the enclosing scope after the for-yield expression completes. In for-do, the `__for_coll` mutable binding via `scope.bind_mutable()` (line 180) adds it to the mutable bindings that get threaded through the loop infrastructure, and the original variable's scope is managed by the loop's `pre_scope` save/restore (`for_iterator.rs:206`). In for-yield, there is no `pre_scope` save/restore (for-yield is an expression, not a statement block), and the original variable may still be referenced by the AIMS backward analysis as a "defined but not consumed" variable in the post-loop scope.

3. **AIMS double-dec** (`realize/walk.rs:308-345`): The AIMS backward analysis (`emit_defined_dead`) emits `RcDec` for variables that are defined but never used. If the source collection variable is visible in a post-loop block (because the for-yield expression doesn't consume it from the enclosing scope), the analysis sees it as "defined but dead" and emits an extra `RcDec`. Combined with (a) the dec from `ori_iter_drop` (via `IterState::Drop`) and (b) the dec from the collection's own last-use cleanup, this produces 3 decs for 2 incs.

4. **Observable failure**: The third dec decrements below zero, triggering a double-free (or an RC underflow abort if `ORI_RT_DEBUG=1` is enabled).

**RC trace comparison** (for `[str]` with 3 elements):

For-do (correct):
```
alloc: list_data [rc=1]               # list construction
rc_inc: list_data [rc=2]              # emit_list_iter gives iterator its ref
ori_iter_from_list(data, elem_dec_fn=NULL)
... 3x iter_next ...
ori_iter_drop -> ori_buffer_rc_dec    # rc=2->1 (no elem cleanup: NULL)
RcDec(list_data)                      # rc=1->0, elem_dec_fn runs, elements cleaned, buffer freed
```

For-yield (BROKEN):
```
alloc: list_data [rc=1]               # list construction
rc_inc: list_data [rc=2]              # emit_list_iter gives iterator its ref
ori_iter_from_list(data, elem_dec_fn=NULL)
... 3x iter_next ...
ori_iter_drop -> ori_buffer_rc_dec    # rc=2->1 (no elem cleanup: NULL)
RcDec(list_data)                      # rc=1->0, elem cleanup (NULL = skipped!), buffer freed
RcDec(list_data)                      # DOUBLE-FREE: rc=0->-1
```

**Root cause summary**: For-yield's `prepare_iterator()` / `lower_for_yield_iterator()` attempts to replicate the `__for_coll` pattern from for-do by threading the collection as a header block param and emitting a dummy let. But this is insufficient because the original variable is not removed from the enclosing scope -- the for-do path uses `scope.bind_mutable()` with the `__for_coll` name which is a separate scope entry from the user's collection variable, and the user's variable dies at the `.iter()` call. The for-yield path keeps the user's variable alive, and the AIMS backward analysis emits an extra dec for it.

**Failed approaches documented** (for Section 03 reference):
- **(a) Broad iter_element_defs suppression**: Suppressing RcDec for all variables with iterator-element projections also suppresses legitimate cleanup, causing leaks.
- **(b) Direct dummy reference in exit block**: Already implemented in current code (line 337-341), but insufficient because it only affects the block-param copy, not the original scope variable.
- **(c) Scope shadowing**: Rebinding the original variable name to a different ArcVarId after `.iter()` -- fragile and doesn't interact correctly with AIMS backward analysis which operates on ArcVarIds, not names.
- **(d) Phantom threading without scope isolation**: Current implementation -- threads the collection through header params but doesn't isolate it from the enclosing scope.

- [x] Reproduce the double-free with `ORI_TRACE_RC=1` on a for-yield over `[Option<str>]` — reproduced: `/tmp/test_option_str_yield.ori` with heap strings (>23 chars) crashes AOT with exit code 134 (SIGABRT). RC trace shows: alloc→inc(rc=2)→inc(rc=3), 3 iterations with per-iter dec+inc, then in exit block: dec(rc=2)→dec(rc=1)→FREE(rc=0) with element cleanup, then DOUBLE-FREE on the already-freed address. For-do version (`/tmp/test_option_str_do.ori`) completes successfully with clean RC balance. (2026-03-18)
- [x] Dump the ARC IR with `ORI_DUMP_AFTER_ARC=1` and annotate each RcInc/RcDec — dumped both for-yield and for-do. For-yield: bb0 has 2 `RcInc %5` + 1 `RcInc %7` (alias), bb2 has per-iter `RcDec %5`, bb3 (exit) has `RcDec %5` + `RcDec %29` (alias), bb12 (post-loop) has `RcDec %44` (alias of `%5`) — the spurious extra dec. For-do: phantom threading via `%12`/`%13` block params keeps decs balanced, no post-loop dec. (2026-03-18)
- [x] Count inc/dec pairs for source collection — for-yield: 3 incs (2 explicit + 1 per iter loop-back), 3 decs on non-unwind path (bb2 per-iter + bb3 exit + bb12 post-loop), but the bb12 dec is spurious because bb3 already freed the buffer. For-do: 2 incs, 2 decs in exit block only (via phantom-threaded params `%13` and `%12`), balanced. (2026-03-18)
- [x] Identify the specific AIMS rule — `emit_defined_dead` in `realize/walk.rs` (~line 308-345). The backward analysis sees `%5` (source list) as "defined but not consumed" in the post-loop scope (bb12). In for-do, the `__for_coll` phantom binding consumes `%5`'s scope via `scope.bind_mutable()`, so `%5` is not visible post-loop. In for-yield, `%5` escapes the loop scope because for-yield is an expression (no `pre_scope` save/restore), so `emit_defined_dead` adds `RcDec %44 = %5` in bb12. (2026-03-18)
- [x] Record the exact block index and instruction — bb12 (post-loop block), `%44: [str?] [RcPtr] = %5` followed by `RcDec %44 [HeapPtr]`. This is the first dec in bb12, coming after the result list operations. (2026-03-18)
- [x] Verify that removing the extra dec would produce correct results — conceptually verified: with 3 incs and only 2 non-spurious decs (bb2 per-iter + bb3 exit), removing the bb12 `RcDec %44` would leave RC at 0 when bb3 frees the buffer. The for-do version demonstrates this exact balanced behavior without the post-loop dec. (2026-03-18)

---

## 01.R Third Party Review Findings

- None.

---

## 01.N Completion Checklist

- [x] Both bug chains documented with exact file paths, line numbers, and function names — NULL `elem_dec_fn` chain: `list_builtins.rs:115-140` → `sources.rs:27-43` → `state.rs:127-155` → `list_rc.rs:24-38`. For-yield chain: `for_yield.rs:56-92,208-346` → `loops.rs:174-181` → `realize/walk.rs:308-345`. (2026-03-18)
- [x] RC trace for for-do (correct) and for-yield (broken) captured and annotated — for-do: balanced 2 inc + 2 dec, clean free. For-yield: 3 inc but extra dec in post-loop bb12 causes double-free. Traces captured via `ORI_TRACE_RC=1` and `ORI_DUMP_AFTER_ARC=1` on test programs. (2026-03-18)
- [x] Design principle ("any dec may be the final dec") stated and justified — documented in plan text. Justification: the `__for_coll` phantom ordering is an assumption, not an invariant. For-yield, `drop_early()`, iterator adapters, and cross-function passing all violate it. (2026-03-18)
- [x] All four failed approaches for Section 03 documented — (a) broad iter_element_defs suppression, (b) direct dummy reference in exit block, (c) scope shadowing, (d) phantom threading without scope isolation. Each with explanation of why it fails. (2026-03-18)
- [x] Element type classification (affected vs unaffected by NULL elem_dec_fn) documented — affected: str, [T], closures, structs with Drop fields, Option/Result with fat-pointer payloads. Unaffected: int, float, bool, char, byte, void. (2026-03-18)
- [x] `__for_coll` phantom mechanism fully explained — for-do: `scope.bind_mutable()` at `loops.rs:180` creates `__for_coll_N`, threaded through loop header→body→latch→exit as block param, dummy Let after `ori_iter_drop` ensures collection's RcDec is last. For-yield: attempts same pattern but `%5` (original var) escapes loop scope because for-yield has no `pre_scope` save/restore. (2026-03-18)
- [x] Map and Str iterator paths documented — Map: `emit_map_iter()` at `map_builtins.rs:340-344` passes NULL for `key_dec_fn`/`val_dec_fn`, no `__for_coll` phantom (only List|Set). Str: `emit_str_iter()` at `string_builtins.rs:203-207` calls `ori_iter_from_str`, correctly uses no `elem_dec_fn` (chars are scalar). (2026-03-18)
- [x] Code changes limited to doc comment cleanup (STYLE items in 01.1) — no functional changes. Analysis complete. (2026-03-18)

---

## Section 01 Exit Criteria

Both bug chains are fully documented from root cause to observable failure. The RC trace comparison between for-do and for-yield is captured. The design principle ("any dec may be the final dec") is established. All four failed approaches for the for-yield fix are documented with clear explanations. This analysis provides the foundation for the fixes in Sections 02 and 03.
