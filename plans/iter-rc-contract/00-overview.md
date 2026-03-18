---
plan: "iter-rc-contract"
title: "Iterator-Collection RC Ownership Contract: Exhaustive Implementation Plan"
status: not-started
supersedes:
  - "plans/fat-pointer-hardening/ (Section 01 — iterator ownership)"
references:
  - "plans/fat-pointer-hardening/section-01-iterator-ownership.md"
  - "plans/rc-header-elem-dec/"
---

# Iterator-Collection RC Ownership Contract: Exhaustive Implementation Plan

## Mission

Establish a correct, complete, and tested ownership contract between iterators and their source collections in the Ori compiler's AOT pipeline. Currently, the contract has two critical bugs: (1) iterator `elem_dec_fn` is hardcoded NULL, so element cleanup fails when `ori_iter_drop` does the final buffer RC dec; (2) the for-yield lowering doesn't properly scope the source collection's lifetime, causing the AIMS pipeline to emit a spurious extra `RcDec` (double-free). This plan fixes both bugs and audits the full for-do/for-yield parity to prevent similar issues.

## Architecture

```
Ori Source: for item in collection do/yield { body }
                    |
                    v
        +----------------------+
        |  ARC Lowering        |
        |  lower_for()         |
        |  +-- for-do path     |  <- __for_coll phantom + mutable var threading
        |  |   lower_for_iterator()
        |  +-- for-yield path  |  <- MISSING phantom, MISSING proper scoping
        |      prepare_iterator()
        |      lower_for_yield_iterator()
        +----------------------+
                    |
                    v
        +----------------------+
        |  AIMS Pipeline       |
        |  realize_rc_reuse()  |
        |  +-- emit_defined_dead   <- iter_element_defs suppression
        |  +-- emit_last_use_decs  <- project-borrowed handling
        |  +-- edge_cleanup        <- Switch RcDec/RcInc pairs
        +----------------------+
                    |
                    v
        +----------------------+
        |  LLVM Codegen        |
        |  emit_list_iter()    |  <- elem_dec_fn: NULL -> get_or_generate_elem_dec_fn()
        |  emit_map_iter()     |  <- key_dec_fn/val_dec_fn: NULL -> get_or_generate_elem_dec_fn()
        |  ArcIrEmitter        |
        +----------------------+
                    |
                    v
        +----------------------+
        |  Runtime (ori_rt)    |
        |  ori_iter_from_list  |  <- stores elem_dec_fn in IterState
        |  ori_iter_from_map   |  <- stores key_dec_fn/val_dec_fn in IterState
        |  IterState::Drop     |  <- calls ori_buffer_rc_dec(data, ..., elem_dec_fn)
        |  drop_elements_and_free  <- calls elem_dec_fn per element when RC->0
        +----------------------+
```

## Design Principles

1. **Any dec may be the final dec.** The `elem_dec_fn` must be set correctly on ALL paths that call `ori_buffer_rc_dec` -- both the AIMS pipeline's explicit `RcDec` instructions AND `ori_iter_drop`'s internal dec. The previous design assumed the AIMS dec would always be the final one (via `__for_coll` ordering), but for-yield breaks this assumption.

2. **For-do and for-yield must have identical RC semantics.** Both paths create iterators from collections, consume elements, and clean up. If the RC contract works for one, it must work for both. Any structural difference (phantom bindings, block param threading, scope restoration) that affects RC correctness is a bug.

3. **The AIMS pipeline must not emit more decs than incs.** Every `RcDec` emitted by the AIMS pipeline must correspond to a matching `RcInc` or allocation. The `iter_element_defs` mechanism suppresses spurious decs for iterator-extracted elements, but the source collection also needs correct scoping to prevent extra decs.

## Relationship to `rc-header-elem-dec` Plan

The `plans/rc-header-elem-dec/` plan proposes a DIFFERENT solution to the same root cause: extending the RC allocation header from 16 to 24 bytes to store the `elem_dec_fn` in the header itself. That approach solves the problem at the runtime layer (any call to `ori_buffer_rc_dec` reads the `elem_dec_fn` from the header, regardless of what the caller passes). This plan (`iter-rc-contract`) solves it at the codegen layer by passing the real `elem_dec_fn` at all call sites.

**This plan is simpler and sufficient for the current bugs.** The `rc-header-elem-dec` plan may still be pursued later for defense-in-depth (belt-and-suspenders), but it is NOT required for correctness if this plan is implemented fully. The two plans are complementary, not conflicting.

**Decision:** This plan takes priority. If this plan is completed, `rc-header-elem-dec` becomes a hardening measure, not a bug fix.

## Section Dependency Graph

```
01 Root Cause Analysis
 |
02 Fix elem_dec_fn  <---- independent of 03
 |                         |
03 Fix For-Yield RC  <--- requires 02 (real elem_dec_fn needed for correct cleanup)
 |
04 Parity Audit      <--- requires 02+03
 |
05 Test Matrix       <--- requires 02+03
 |
06 Verification      <--- requires all
```

- Section 01 is pure analysis (no code changes).
- Section 02 (elem_dec_fn) is a codegen-only change -- safe to land independently.
- Section 03 (for-yield RC) requires Section 02 because the proper elem_dec_fn is needed for correct cleanup regardless of which dec reaches zero.
- Sections 04 and 05 are audit/testing that validate 02+03.
- Section 06 is the merge gate.

## Implementation Sequence

```
Phase 0 - Analysis
  +- 01: Root cause documentation (no code changes)

Phase 1 - Foundation Fix
  +- 02: Pass real elem_dec_fn in emit_list_iter() AND real key_dec_fn/val_dec_fn in emit_map_iter()
  Gate: [str] for-yield tests pass, {str: int} map tests pass, existing for-do tests still pass

Phase 2 - Structural Fix  [CRITICAL PATH]
  +- 03: Fix for-yield source collection RC scoping
  Gate: [Option<str>] for-yield produces zero leaks AND zero double-frees

Phase 3 - Audit & Hardening
  +- 04: For-do / for-yield parity audit
  +- 05: Test matrix (element type x pattern x loop variant)
  Gate: All matrix tests pass in debug+release with ORI_CHECK_LEAKS=1

Phase 4 - Verification
  +- 06: Full test suite, Valgrind, merge gate
  Gate: ./test-all.sh green, clippy green, zero Valgrind errors on test programs
```

## Known Bugs

| Bug | Root Cause | Fix Location | Status |
|-----|-----------|-------------|--------|
| `[Option<str>]` for-yield leaks str payloads | Iterator `elem_dec_fn` hardcoded NULL in `emit_list_iter()` | Section 02 | Not Started |
| `[Option<str>]` for-yield double-frees source list | AIMS emits 3 decs for 2 incs -- source list alive in post-loop scope | Section 03 | Not Started |
| `{str: int}` map iteration leaks str keys | `emit_map_iter` passes NULL for both `key_dec_fn`/`val_dec_fn` -- identical root cause to list bug | Section 02 | Not Started |
| `[[int]]` for-do double-freed elements | `emit_defined_dead` emitted RcDec for `__iter_next` projections | Fixed (iter_element_defs) | Guarded |
| `__for_coll` name collision in nested loops | Single name `__for_coll` shadowed in nested for-loops | Fixed (__for_coll_N) | Guarded |

## Known Gaps

| Gap | Description | Impact | Fix Location | Status |
|-----|-----------|--------|-------------|--------|
| `break`/`continue` in for-yield AOT | `lower_for_yield_iterator` does not set up `loop_ctx`, so `break`/`continue`/`continue value` in for-yield body will not work in AOT backend. Spec (Clause 16.10) allows all three. | Test matrix P2 (break) and P8 (continue) for for-yield BLOCKED until lowering is fixed. Tests must use `#skip` or lowering must be fixed first. | Section 03.5 (add `LoopContext` setup in `lower_for_yield_iterator`) | **Mandatory -- fix in Section 03 or mark P2/P8 for-yield tests as `#skip("for-yield break/continue not yet lowered")` with tracking** |
| `needs_phantom` excludes Map | Both for-do (`loops.rs:174`) and for-yield (`for_yield.rs:85`) only apply phantom for `List \| Set`, not Map. Map uses different cleanup (`ori_map_buffer_rc_dec` with `key_dec_fn`/`val_dec_fn`). | Map iteration with str keys/values leaks or double-frees -- `emit_map_iter` passes NULL for both dec fns. | Section 02.3 (pass real `key_dec_fn`/`val_dec_fn` in `emit_map_iter`) | **Mandatory -- fix in Section 02** |
| `emit_map_iter` passes NULL dec fns | `map_builtins.rs:343-344` passes `const_null_ptr()` for both `key_dec_fn` and `val_dec_fn`. Same root cause as the list `elem_dec_fn` NULL bug. `IterState::Map::Drop` calls `ori_map_buffer_rc_dec` with these NULLs, so map element cleanup fails. | Maps with str keys or str values will leak when the iterator's Drop is the final dec. | Section 02.3 (parallel fix to `emit_list_iter`) | **Mandatory -- fix in Section 02** |
| Set shares `emit_list_iter` path | `builtins/mod.rs:371` routes `TypeInfo::Set` through `emit_list_iter`. The `elem_dec_fn` fix in Section 02 automatically covers sets. | No additional code change needed, but Set-specific tests are required. | Section 05 (add Set element type tests) | Covered by Section 02 fix |
| `walk.rs` over 500-line limit (595 lines) | `aims/realize/walk.rs` is 595 lines, exceeding the 500-line limit. Contains 6 functions that could be split into submodules. | If Section 03 modifies this file, split it first. | Section 03 (split before modifying) | **Mandatory if touched** |
| `realize/mod.rs` at 500-line limit (505 lines) | `aims/realize/mod.rs` is 505 lines, at the limit boundary. | Same as above -- split before modifying. | Section 03 (split before modifying if needed) | **Mandatory if touched** |
| `transfer/mod.rs` over 500-line limit (516 lines) | `aims/transfer/mod.rs` is 516 lines, exceeding the 500-line limit. | Same as above -- split before modifying. | Section 03 (split before modifying if needed) | **Mandatory if touched** |
| `helpers.rs` merged doc comment | `emit_rc/helpers.rs:177-196` has two doc comments concatenated onto `collect_iter_element_defs`. The first paragraph belongs to `collect_project_borrowed_defs` (line 236, which has no doc comment). | Misleading docs, `collect_project_borrowed_defs` appears undocumented. | Section 03.3 (fix when touching helpers.rs) | **Mandatory** |
| `map_builtins.rs` misleading doc | `emit_map_iter()` doc comment says "Null elem_dec functions prevent double-free" -- this is factually wrong. The NULL functions CAUSE leaks. | Misleading documentation. | Section 02.3 (update doc when fixing NULL dec fns) | **Mandatory** |

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Root Cause Analysis & Design | `section-01-root-cause.md` | Not Started |
| 02 | Fix Iterator elem_dec_fn (List, Map, Set) | `section-02-elem-dec-fn.md` | Not Started |
| 03 | Fix For-Yield RC Scoping | `section-03-for-yield-rc.md` | Not Started |
| 04 | For-Do / For-Yield Parity Audit | `section-04-parity-audit.md` | Not Started |
| 05 | Comprehensive Test Matrix | `section-05-test-matrix.md` | Not Started |
| 06 | Verification & Merge Gate | `section-06-verification.md` | Not Started |
