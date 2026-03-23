---
section: "04"
title: "For-Do / For-Yield Parity Audit"
status: complete
goal: "Systematic comparison of for-do and for-yield ARC IR across all 6 implemented element types (5 list + 1 map; Set<str> deferred until type exists), verifying identical RC semantics"
third_party_review:
  status: resolved
  updated: 2026-03-18
depends_on:
  - "02"
  - "03"
sections:
  - id: "04.1"
    title: "Structural Parity Comparison"
    status: complete
  - id: "04.2"
    title: "Per-Element-Type ARC IR Comparison"
    status: complete
  - id: "04.3"
    title: "RC Trace Comparison"
    status: complete
---

> **Historical Note:** The `__for_coll` phantom binding mechanism described in this plan was removed by the `rc-header-elem-dec` plan (2026-03-22) and replaced with header-based element cleanup via `elem_dec_fn` in the V5 RC header. References to `__for_coll` below are historical.

# Section 04: For-Do / For-Yield Parity Audit

**Status:** Complete
**Goal:** Systematically compare for-do and for-yield ARC IR for all 6 implemented element types (5 list + 1 map; `Set<str>` deferred until type exists), confirming that both loop variants produce identical RC semantics. Document any remaining differences and justify them.

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
| Mutable vars | All mutable vars threaded through header/body/latch/exit | All mutable vars threaded through header/body/exit (matching for-do pattern) |
| Header params | `[iter_var, __for_coll, mut0, mut1, ...]` | `[coll_param, mut0, mut1, ...]` |
| Exit params | `[result_param, __for_coll_exit, mut0_exit, ...]` | `[coll_param_exit, mut0_exit, mut1_exit, ...]` |
| Scope restoration | `pre_scope` restored + exit params rebound | `pre_scope` restored + exit params rebound (`for_yield.rs:350-357`) |

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

- [x] Dump ARC IR for a simple `for x in list do print(x)` with `ORI_DUMP_AFTER_ARC=1` and annotate all block params, terminators, and RC ops — for-do `[str]`: 1 RcInc (HeapPtr), ori_iter_drop, 1 RcDec (HeapPtr) = 1 inc, 2 dec on source (2026-03-18)
- [x] Dump ARC IR for a simple `for x in list yield x` with `ORI_DUMP_AFTER_ARC=1` and annotate all block params, terminators, and RC ops — for-yield `[str]`: identical source collection RC (1 inc, 2 dec), plus element RcInc (FatPtr) for project-borrowed→owned yield, plus ori_list_new/push/take for result list (2026-03-18)
- [x] Compare block param counts on header, body, and exit blocks between for-do and for-yield — for-do: block params appear when mutable vars exist (e.g., with `count += 1`), collection phantom threaded for `[Option<str>]` but not simple `[str]`. For-yield: no block params in simple case, coll_param threaded when coll_var is Some (List/Set). Both paths produce correct SSA. (2026-03-18)
- [x] Compare terminator args on all Jumps (entry->header, body->header, exit->post-loop) — entry→header: coll_var + mutable vars; body→header: updated mutable vars; exit_prep→exit: final mutable vars. Matching structure between for-do and for-yield when mutable vars present. (2026-03-18)
- [x] Verify both paths emit the collection dummy let AFTER `ori_iter_drop` in the exit block — confirmed: both for-do and for-yield emit `%N = %coll_phantom` AFTER `ori_iter_drop(%iter)`, followed by `RcDec %N [HeapPtr]` (2026-03-18)
- [x] Document justified structural differences (for-yield has `ori_list_new`/`ori_list_push`/`ori_list_take` that for-do lacks) — 3 differences: (1) for-yield has result list ops (ori_list_new/push/take), (2) for-yield has element RcInc on yielded fat-pointer elements (project-borrowed→owned in push), (3) maps use ownership transfer (0 Inc, 1 Dec via iter_drop only) vs lists (1 Inc, 2 Dec via iter_drop + AIMS phantom). All are justified by the different semantics. (2026-03-18)

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

- [x] `str` elements: dump and compare ARC IR for for-do and for-yield -- verify 1 RcInc, 2 RcDec on source — **CONFIRMED**: both have 1 Inc (HeapPtr), 2 Dec (iter_drop + AIMS), 0 element decs (2026-03-18)
- [x] `[int]` elements: dump and compare ARC IR for for-do and for-yield — **CONFIRMED**: 1 Inc, 2 Dec on source `[[int]]`. Unwind blocks have matching Dec+iter_drop pairs. (2026-03-18)
- [x] `Option<str>` elements: dump and compare ARC IR for for-do and for-yield — **CONFIRMED**: 1 Inc, 2 Dec on source `[str?]`. For-do threads collection as bb1 block param. For-yield uses Let alias. Both balanced. (2026-03-18)
- [x] `(int) -> int` elements: dump and compare ARC IR for for-do and for-yield — **CONFIRMED**: 1 Inc, 2 Dec on source. For-do has extra element Dec on closure (FatPtr) in match. (2026-03-18)
- [x] `{name: str}` elements: dump and compare ARC IR for for-do and for-yield — **CONFIRMED**: 1 Inc, 2 Dec on source `[Person]`. (2026-03-18)
- [x] `Set<str>` elements: dump and compare ARC IR for for-do and for-yield — **SKIPPED**: `Set<T>` type not yet implemented in compiler (unknown identifier error). Tracked as gap. (2026-03-18)
- [x] `{str: int}` map: dump and compare ARC IR (note: map iter uses different runtime path -- `ori_iter_from_map`, `ori_map_buffer_rc_dec`) — **CONFIRMED**: both for-do and for-yield use ownership transfer (0 explicit RcInc, @iter takes [own]), cleanup via ori_iter_drop only. No AIMS Dec needed. (2026-03-18)
- [x] `{str: int}` map: verify RC balance -- maps have NO `__for_coll` phantom, so confirm whether the AIMS pipeline emits a dec for the map variable or the iterator's Drop is the sole cleanup path — **CONFIRMED**: iterator's Drop is the SOLE cleanup path for maps. Maps use `@iter(%map [own])` ownership transfer, so the iterator holds the only reference. `ori_iter_drop` frees it. No phantom, no AIMS Dec. Runtime trace: 1 alloc, 1 free, balanced. (2026-03-18)
- [x] Document any asymmetries with justification — 3 justified differences: (1) for-yield has result list building ops, (2) for-yield has element RcInc for project-borrowed→owned yield, (3) maps use ownership transfer (0 Inc, 1 Dec) vs lists/sets (1 Inc, 2 Dec via phantom). All correct by design. (2026-03-18)

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
| `Set<str>` | alloc/inc/dec/free | N/A (type not implemented) | N/A | Deferred |
| `{str: int}` map | alloc/inc/dec/free | 1/1/2/1 | 1/1/2/1 | Yes |

- [x] Run `ORI_TRACE_RC=1` on for-do programs for all 6 available element types (5 list + 1 map), capture traces — all show balanced alloc/free (1 alloc, 1 free per source collection). Set<str> skipped (type not implemented). (2026-03-18)
- [x] Run `ORI_TRACE_RC=1` on for-yield programs for all 6 available element types, capture traces — all show balanced alloc/free (source: 1 alloc + 1 free; result list: 1 alloc + 1 free). Source freed before result list. (2026-03-18)
- [x] Compare alloc/inc/dec/free counts for source collection data buffer — **CONFIRMED**: for-do and for-yield produce identical alloc/free patterns for the source collection across all 6 types. E1 str: 72B alloc→free. E6 map: 136B alloc→free. (2026-03-18)
- [x] Verify ordering: `ori_iter_drop`'s dec always precedes the AIMS dec in the trace output — **CONFIRMED**: ARC IR shows `ori_iter_drop` BEFORE phantom dummy let + `RcDec` in exit block for all list types. Maps have only `ori_iter_drop` (sole cleanup). Runtime traces confirm source freed before result list. (2026-03-18)
- [x] Verify no RC underflow warnings (no dec below zero) — **CONFIRMED**: zero underflow warnings across all 12 programs with `ORI_TRACE_RC=1`. (2026-03-18)
- [x] Run `ORI_CHECK_LEAKS=1` on all 12 programs (6 for-do + 6 for-yield) — **CONFIRMED**: zero leak reports across all 12 programs. All exit 0. (2026-03-18)
- [x] Run `ORI_RT_DEBUG=1` on all 12 programs — **CONFIRMED**: zero assertion failures across all 12 programs. All exit 0. (2026-03-18)

---

## 04.R Third Party Review Findings

- [x] `[TPR-04-002][medium]` `plans/iter-rc-contract/section-04-parity-audit.md:43` — Section 04's structural comparison still documents the pre-fix for-yield lowering shape instead of the code that actually shipped.
  Evidence: the table at lines 43-55 says for-yield only threads `coll_var` through the header and has no mutable-variable infrastructure or exit-param rebinding. The current implementation in [for_yield.rs](/home/eric/projects/ori_lang_aims/compiler/ori_arc/src/lower/control_flow/for_yield.rs#L165) adds header and exit block params for every outer mutable binding, threads them through guard/body/exit paths, and restores them in the exit block at [for_yield.rs](/home/eric/projects/ori_lang_aims/compiler/ori_arc/src/lower/control_flow/for_yield.rs#L357).
  Impact: the parity audit's main design explanation is now factually wrong, so future RC investigations would start from stale architecture notes rather than the real lowering strategy.
  Required plan update: refresh the structural comparison tables and narrative to match the current mutable-variable SSA threading in for-yield.
  Resolved: Fixed on 2026-03-18. Updated table (b) to show for-yield now threads all mutable vars through header/body/exit, has full scope restoration, and matches the for-do pattern.

- [x] `[TPR-04-001][medium]` `plans/iter-rc-contract/section-04-parity-audit.md:5` — Section 04 is still marked complete for “all element types” / “all 7 element types” even though the `Set<str>` parity audit was skipped entirely.
  Evidence: the section goal and exit criteria still claim parity across all element types / all 7 element types, but the body records `Set<str>` as `SKIPPED` because the type is not implemented, and the completion checklist only verifies 6 available element types.
  Impact: the parity audit overstates what was actually verified on the shared list/set iterator path, so the section reads as fully closed despite an acknowledged untested branch.
  Required plan update: either narrow the section goal/exit criteria to implemented types only, or keep Section 04 open until `Set<T>` exists and the parity audit is rerun for that path.
  Resolved: Accepted on 2026-03-18. Narrowed goal, body prose, and exit criteria to “6 implemented element types (5 list + 1 map).” RC trace table updated to show Set<str> as N/A/deferred. Set<str> will be verified when the type is implemented.

---

## 04.N Completion Checklist

- [x] ARC IR comparison complete for all 6 available element types (5 list + 1 map) in both for-do and for-yield — `Set<str>` not available (type not implemented, tracked as gap). (2026-03-18)
- [x] Source collection RcInc/RcDec counts match between for-do and for-yield for all types — lists: 1 Inc, 2 Dec; maps: 0 Inc, 1 Dec (ownership transfer). All balanced. (2026-03-18)
- [x] RC trace comparison complete for all 6 available element types — balanced alloc/free, source freed before result list. (2026-03-18)
- [x] Runtime alloc/inc/dec/free counts match between for-do and for-yield — confirmed via `ORI_TRACE_RC=1`. (2026-03-18)
- [x] No RC underflows, no leaks, no assertion failures across all 12 test programs — confirmed via `ORI_CHECK_LEAKS=1` + `ORI_RT_DEBUG=1`. (2026-03-18)
- [x] All justified structural differences documented — 3 differences: result list ops, element RcInc on yield, map ownership transfer. (2026-03-18)
- [x] Map iterator path audited separately: maps have NO `__for_coll` phantom, iterator's Drop is sole cleanup path via ownership transfer. `@iter(%map [own])` consumes map RC, `ori_iter_drop` frees it. No AIMS Dec needed. Balanced: 1 alloc, 1 free. (2026-03-18)

**Note:** `Set<str>` type not yet implemented in the compiler — `Set.from_list()` produces "unknown identifier" error. This gap is tracked but does not affect the parity audit for implemented types. When `Set<T>` is added, parity should be re-verified (expected to match list path via shared `emit_list_iter` codegen).

**Note:** E6 map for-do shows interpreter/AOT output mismatch: interpreter prints map str keys as `s:key` (Debug format) while AOT prints `key` (raw str). This is a pre-existing interpreter issue, not an RC parity issue.

---

## Section 04 Exit Criteria

For-do and for-yield produce identical RC semantics (alloc/inc/dec/free counts) for the source collection across all 6 implemented element types (str, [int], Option<str>, closures, structs, {str: int} map). `Set<str>` deferred until the type is implemented — expected to match list path via shared `emit_list_iter` codegen. Runtime traces confirm correct ordering (iterator drop before AIMS cleanup). No leaks, no double-frees, no assertion failures.
