---
section: "01"
title: "Root Cause Analysis & Design"
status: not-started
goal: "Document the full causal chain from codegen through runtime that produces leaked elements and double-frees in iterator-based loops, establishing the shared understanding needed for Sections 02-06"
third_party_review: false
sections:
  - id: "01.1"
    title: "NULL elem_dec_fn Bug Chain"
    status: not-started
  - id: "01.2"
    title: "For-Yield Spurious RcDec Bug Chain"
    status: not-started
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

- [ ] Trace the full codegen path: `lower_for()` -> `lower_for_yield()` -> `prepare_iterator()` -> ARC IR `Apply("iter", ...)` -> LLVM `emit_list_iter()` -> `ori_iter_from_list()` call
- [ ] Trace the runtime drop path: `ori_iter_drop()` -> Rust `Drop for IterState` -> `ori_buffer_rc_dec()` -> `drop_elements_and_free()` -> element cleanup skipped (NULL)
- [ ] Document which element types are affected (str, [T], closures, structs with Drop fields, Option/Result with fat-pointer payloads)
- [ ] Document which element types are NOT affected (int, float, bool, char, byte, void -- scalar types with no RC children)
- [ ] Verify that `get_or_generate_elem_dec_fn()` in `element_fn_gen.rs` handles all affected element types listed above
- [ ] Confirm that the for-do `__for_coll` phantom still produces correct results even with the real `elem_dec_fn` (the AIMS dec is idempotent -- calling `elem_dec_fn` on already-cleaned elements is safe because the inner RC will already be zero, making the inner dec a no-op)
- [ ] Document the parallel map bug: `emit_map_iter()` (`map_builtins.rs:343-344`) passes NULL for both `key_dec_fn` and `val_dec_fn` -- identical root cause. `IterState::Map::Drop` calls `ori_map_buffer_rc_dec()` with these NULLs. Maps with str keys/values have no working element cleanup path AND no `__for_coll` phantom workaround (phantom only covers `List | Set`).
- [ ] Document set coverage: `emit_auto_iter()` routes `TypeInfo::Set` through `emit_list_iter()` (`builtins/mod.rs:371`), so the list `elem_dec_fn` fix automatically covers sets. No separate set code change needed.
- [ ] Document Str iterator path: `emit_str_iter()` calls `ori_iter_from_str(str_ptr)`. `IterState::Str::Drop` calls `ori_buffer_rc_dec(data, 0, len, 1, None)` when `owns_data` is true. The `elem_dec_fn` is correctly `None` because char codepoints are scalar (no RC children). Str iteration is NOT affected by the NULL `elem_dec_fn` bug. The `__for_coll` phantom also excludes Str (`loops.rs:174` matches `List | Set` only). This is correct -- Str iterator Drop passes `None` for `elem_dec_fn` and `cap=len` (byte length), which frees the string data buffer when RC reaches zero.

### Codebase Cleanup (fix alongside analysis)

- [ ] **STYLE**: Split merged doc comment in `helpers.rs:177-196` -- the doc block for `collect_project_borrowed_defs` is concatenated with the doc for `collect_iter_element_defs`. Separate into two `///` doc comments, one on each function.
- [ ] **STYLE**: Add missing `///` doc comment to `collect_project_borrowed_defs` at `helpers.rs:236`.
- [ ] **DOCS**: Update doc comment on `emit_map_iter` (`map_builtins.rs:325-327`) -- current comment says "Null elem_dec functions prevent double-free" which is factually wrong (NULL causes leaks). Correct after the Section 02.3 fix.
- [ ] **TRACKING**: Verify `for_yield.rs:364` TODO reference to `type_strategy_registry/section-11` -- confirm this plan exists. If not, create a tracking item.

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

- [ ] Reproduce the double-free with `ORI_TRACE_RC=1` on a for-yield over `[str]` and capture the full trace output
- [ ] Dump the ARC IR with `ORI_DUMP_AFTER_ARC=1` and annotate each RcInc/RcDec with its source (emit_defined_dead, emit_last_use_decs, or edge_cleanup)
- [ ] Count inc/dec pairs for the source collection variable in both for-do and for-yield ARC IR -- confirm for-do has 2 decs and for-yield has 3
- [ ] Identify the specific AIMS rule (emit_defined_dead vs emit_last_use_decs vs edge_cleanup) that emits the spurious third dec
- [ ] Record the exact block index and instruction index where the extra dec appears in the ARC IR dump
- [ ] Verify that manually removing the extra dec from the ARC IR (by editing the dump) would produce correct results

---

## 01.R Third Party Review Findings

- None.

---

## 01.N Completion Checklist

- [ ] Both bug chains documented with exact file paths, line numbers, and function names
- [ ] RC trace for for-do (correct) and for-yield (broken) captured and annotated
- [ ] Design principle ("any dec may be the final dec") stated and justified
- [ ] All four failed approaches for Section 03 documented with explanations of why each fails
- [ ] Element type classification (affected vs unaffected by NULL elem_dec_fn) documented
- [ ] `__for_coll` phantom mechanism fully explained (how it works in for-do, why it fails in for-yield)
- [ ] Map and Str iterator paths documented (how they differ from list path)
- [ ] No code changes -- this section is pure analysis

---

## Section 01 Exit Criteria

Both bug chains are fully documented from root cause to observable failure. The RC trace comparison between for-do and for-yield is captured. The design principle ("any dec may be the final dec") is established. All four failed approaches for the for-yield fix are documented with clear explanations. This analysis provides the foundation for the fixes in Sections 02 and 03.
