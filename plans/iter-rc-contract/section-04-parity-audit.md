---
section: "04"
title: "For-Do / For-Yield Parity Audit"
status: not-started
goal: "Systematic comparison of for-do and for-yield ARC IR across all element types, verifying identical RC semantics"
third_party_review: false
depends_on:
  - "02"
  - "03"
sections:
  - id: "04.1"
    title: "Structural Parity Comparison"
    status: not-started
  - id: "04.2"
    title: "Per-Element-Type ARC IR Comparison"
    status: not-started
  - id: "04.3"
    title: "RC Trace Comparison"
    status: not-started
---

# Section 04: For-Do / For-Yield Parity Audit

**Status:** Not Started
**Goal:** Systematically compare for-do and for-yield ARC IR for all element types, confirming that both loop variants produce identical RC semantics. Document any remaining differences and justify them.

**Context:** After Sections 02 and 03 fix the two bugs, this audit verifies the fixes produce correct results across the full element-type spectrum. For-do and for-yield have different purposes (side effects vs list building), so their ARC IR will differ structurally. But the RC operations on the SOURCE COLLECTION must be semantically identical: 1 alloc, 1 inc (for iterator), 2 decs (iterator drop + AIMS cleanup).

---

## 04.1 Structural Parity Comparison

**File(s):** `compiler/ori_arc/src/lower/control_flow/loops.rs` (for-do dispatch), `compiler/ori_arc/src/lower/control_flow/for_yield.rs` (for-yield dispatch), `compiler/ori_arc/src/lower/control_flow/for_loops/for_iterator.rs` (for-do iterator loop)

Compare the structural elements of for-do and for-yield for each dimension:

### (a) __for_coll Phantom Presence

| Dimension | For-Do | For-Yield |
|-----------|--------|-----------|
| Phantom binding | `__for_coll_N` as mutable scope entry (`loops.rs:180` via `scope.bind_mutable()`, only for `List \| Set` tags -- `loops.rs:174`) | `coll_var` returned from `prepare_iterator()` (`for_yield.rs:85-90`, also only `List \| Set`) |
| Scope mechanism | `scope.bind_mutable()` -- adds to mutable bindings list | Block param on header only -- not in mutable bindings |
| Threading path | Mutable var threading (header/body/latch/exit params) | Explicit header block param + dummy let in exit |
| Name collision | Unique `__for_coll_N` counter | No name -- uses `ArcVarId` directly |

### (b) Mutable Variable Threading

| Dimension | For-Do | For-Yield |
|-----------|--------|-----------|
| Mutable vars | All mutable vars threaded through header/body/latch/exit | Only `coll_var` threaded through header (no mutable var infrastructure) |
| Header params | `[iter_var, __for_coll, mut0, mut1, ...]` | `[coll_param]` (if coll_var is Some) |
| Exit params | `[result_param, __for_coll_exit, mut0_exit, ...]` | `[result_param]` (no mutable var flow) |
| Scope restoration | `pre_scope` restored + exit params rebound | No scope restoration needed (for-yield is an expression, not a statement block) |

### (c) Exit Block Structure

| Dimension | For-Do | For-Yield |
|-----------|--------|-----------|
| Iterator drop | `ori_iter_drop(iter_val)` (`for_iterator.rs:185-187`) | `ori_iter_drop(iter_val)` (`for_yield.rs:330-332`) |
| Collection dummy | `Let { Var(__for_coll_exit_param) }` AFTER iter_drop (`for_iterator.rs:196-204`, finds phantom via `starts_with("__for_coll_")`) | `Let { Var(coll_param) }` AFTER iter_drop (`for_yield.rs:337-341`) |
| Result extraction | `result_param` from exit block params | `ori_list_take(list_ptr)` to extract final list |

### (d) AIMS RcDec Count (TARGET after fixes)

| Operation | For-Do | For-Yield |
|-----------|--------|-----------|
| Source collection RcInc | 1 (from `emit_list_iter`) | 1 (from `emit_list_iter`) |
| Source collection RcDec | 2 (iter_drop + AIMS) | 2 (iter_drop + AIMS) |
| Element RcDec | 0 (suppressed by `iter_element_defs`) | 0 (suppressed by `iter_element_defs`) |
| Result list RcInc/Dec | N/A (no result list) | Per result list allocation |

- [ ] Dump ARC IR for a simple `for x in list do print(x)` with `ORI_DUMP_AFTER_ARC=1` and annotate all block params, terminators, and RC ops
- [ ] Dump ARC IR for a simple `for x in list yield x` with `ORI_DUMP_AFTER_ARC=1` and annotate all block params, terminators, and RC ops
- [ ] Compare block param counts on header, body, and exit blocks between for-do and for-yield
- [ ] Compare terminator args on all Jumps (entry->header, body->header, exit->post-loop)
- [ ] Verify both paths emit the collection dummy let AFTER `ori_iter_drop` in the exit block
- [ ] Document justified structural differences (for-yield has `ori_list_new`/`ori_list_push`/`ori_list_take` that for-do lacks)

---

## 04.2 Per-Element-Type ARC IR Comparison

For each element type, compare the ARC IR between for-do and for-yield, focusing on the source collection's RC operations.

### Element Type Matrix

| Element Type | For-Do ARC IR | For-Yield ARC IR | Expected Parity |
|-------------|---------------|------------------|-----------------|
| `str` | 1 RcInc, 2 RcDec on source; 0 elem decs | Same | Full parity |
| `[int]` | 1 RcInc, 2 RcDec on source; 0 elem decs | Same | Full parity |
| `Option<str>` | 1 RcInc, 2 RcDec on source; 0 elem decs | Same | Full parity |
| `(int) -> int` | 1 RcInc, 2 RcDec on source; 0 elem decs | Same | Full parity |
| `{name: str}` | 1 RcInc, 2 RcDec on source; 0 elem decs | Same | Full parity |
| `{str: int}` map | Different runtime path (`emit_map_iter` -> `ori_iter_from_map` -> `IterState::Map::Drop` -> `ori_map_buffer_rc_dec`). After Section 02.3 fix: non-null `key_dec_fn`/`val_dec_fn`. No `__for_coll` phantom for maps (phantom only applies to `List \| Set`). | Same mechanism. | Different cleanup path, but same RC balance contract. Verify map iterator Drop is the only dec if no phantom exists. |
| `Set<str>` | Same as `str` list path (shared `emit_list_iter` codegen) | Same | Full parity |

For each type:
1. Write a minimal for-do program and a minimal for-yield program with the same source collection
2. Dump ARC IR with `ORI_DUMP_AFTER_ARC=1`
3. Count RcInc and RcDec instructions referencing the source collection's `ArcVarId`
4. Verify counts match between for-do and for-yield

- [ ] `str` elements: dump and compare ARC IR for for-do and for-yield -- verify 1 RcInc, 2 RcDec on source
- [ ] `[int]` elements: dump and compare ARC IR for for-do and for-yield
- [ ] `Option<str>` elements: dump and compare ARC IR for for-do and for-yield
- [ ] `(int) -> int` elements: dump and compare ARC IR for for-do and for-yield
- [ ] `{name: str}` elements: dump and compare ARC IR for for-do and for-yield
- [ ] `Set<str>` elements: dump and compare ARC IR for for-do and for-yield
- [ ] `{str: int}` map: dump and compare ARC IR (note: map iter uses different runtime path -- `ori_iter_from_map`, `ori_map_buffer_rc_dec`)
- [ ] `{str: int}` map: verify RC balance -- maps have NO `__for_coll` phantom, so confirm whether the AIMS pipeline emits a dec for the map variable or the iterator's Drop is the sole cleanup path
- [ ] Document any asymmetries with justification (e.g., for-yield has result list ops that for-do lacks)

---

## 04.3 RC Trace Comparison

**File(s):** Run programs with `ORI_TRACE_RC=1` and compare runtime traces.

For each element type, run both the for-do and for-yield versions and compare:
1. Number of `alloc` events for the source collection
2. Number of `rc_inc` events for the source collection's data buffer
3. Number of `rc_dec` events for the source collection's data buffer
4. Number of `free` events for the source collection's data buffer
5. Ordering: `ori_iter_drop`'s dec comes before the AIMS dec

| Element Type | Metric | For-Do | For-Yield | Match? |
|-------------|--------|--------|-----------|--------|
| `str` | alloc/inc/dec/free | 1/1/2/1 | 1/1/2/1 | Yes |
| `[int]` | alloc/inc/dec/free | 1/1/2/1 | 1/1/2/1 | Yes |
| `Option<str>` | alloc/inc/dec/free | 1/1/2/1 | 1/1/2/1 | Yes |
| `(int) -> int` | alloc/inc/dec/free | 1/1/2/1 | 1/1/2/1 | Yes |
| `{name: str}` | alloc/inc/dec/free | 1/1/2/1 | 1/1/2/1 | Yes |
| `Set<str>` | alloc/inc/dec/free | 1/1/2/1 | 1/1/2/1 | Yes |
| `{str: int}` map | alloc/inc/dec/free | 1/1/2/1 | 1/1/2/1 | Yes |

- [ ] Run `ORI_TRACE_RC=1` on for-do programs for all 7 element types (5 list + 1 set + 1 map), capture traces
- [ ] Run `ORI_TRACE_RC=1` on for-yield programs for all 7 element types, capture traces
- [ ] Compare alloc/inc/dec/free counts for source collection data buffer -- confirm they match between for-do and for-yield
- [ ] Verify ordering: `ori_iter_drop`'s dec always precedes the AIMS dec in the trace output
- [ ] Verify no RC underflow warnings (no dec below zero)
- [ ] Run `ORI_CHECK_LEAKS=1` on all 14 programs (7 for-do + 7 for-yield) -- verify zero leak reports
- [ ] Run `ORI_RT_DEBUG=1` on all 14 programs -- verify no assertion failures

---

## 04.R Third Party Review Findings

- None.

---

## 04.N Completion Checklist

- [ ] ARC IR comparison complete for all 7 element types (5 list + 1 set + 1 map) in both for-do and for-yield
- [ ] Source collection RcInc/RcDec counts match between for-do and for-yield for all types
- [ ] RC trace comparison complete for all 7 element types
- [ ] Runtime alloc/inc/dec/free counts match between for-do and for-yield
- [ ] No RC underflows, no leaks, no assertion failures across all 14 test programs
- [ ] All justified structural differences documented
- [ ] Map iterator path audited separately: maps have NO `__for_coll` phantom, so verify whether AIMS emits a dec for the map variable or only the iterator's Drop handles cleanup. Document the exact RC balance (total decs should still equal total incs + allocs).

---

## Section 04 Exit Criteria

For-do and for-yield produce identical RC semantics (alloc/inc/dec/free counts) for the source collection across all 7 element types. Runtime traces confirm correct ordering (iterator drop before AIMS cleanup). No leaks, no double-frees, no assertion failures.
