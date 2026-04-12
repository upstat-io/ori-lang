---
plan: "iterator-element-ownership"
title: "Iterator Element Ownership Protocol: Exhaustive Implementation Plan"
status: not-started
supersedes: []
references:
  - "plans/bug-tracker/section-05-runtime-arc.md"
  - "plans/bug-tracker/fix-BUG-04-039.md"
---

# Iterator Element Ownership Protocol: Exhaustive Implementation Plan

## Mission

Establish a uniform element ownership contract for the C iterator runtime protocol: every element yielded by `IterState::next()` is **owned (+1 RC)** by the consumer. Consumers dec after use; adapters that discard elements dec before discarding. This eliminates BUG-05-003 (leaked owned elements from `map`/`flat_map` adapters) and the broader class of borrow-vs-own ambiguity bugs across all 11 consumer functions, 11 adapter variants, and the AIMS analysis pipeline.

## Mission Success Criteria

- [ ] `ORI_CHECK_LEAKS=1` reports zero leaks for ALL consumer x adapter x RC-type combinations (join, collect, count, any, all, find, for_each, fold, last, rfind, rfold with map/filter/take/skip/enumerate/zip/chain/flatten/cycle/rev on str/[int]/closures)
- [ ] `collect` no longer calls `elem_inc_fn` per element (element already owned; transfer semantics)
- [ ] `filter` and `skip` adapters call `elem_dec_fn` on discarded elements
- [ ] `collect_iter_element_defs` in borrowed_defs.rs is removed or reworked so AIMS treats yielded elements as owned
- [ ] `./test-all.sh` green — no regressions (17,119+ tests)
- [ ] All section success criteria met

## Architecture

```
Source                    Adapter                   Consumer
[str].iter()              .map(f)                   .join(",")
     |                      |                          |
  next_list()            next_mapped()           ori_iter_join()
  copies bytes           transform_fn             reads element
  from list              writes NEW val           push_str(s)
     |                      |                          |
  +++ RC INC +++        (already RC=1)          +++ RC DEC +++
  on the copy           no change needed         after push_str
     |                      |                          |
  out_ptr (owned)       out_ptr (owned)          element freed
```

**Contract:** Every element crossing the yield boundary is a `+1` owned value. Source `next_*` functions inc when copying from borrowed storage. Adapter `next_*` functions that produce new values (map, flat_map) yield them as-is (already RC=1). Adapters that pass through (filter, take, skip, chain) inherit source ownership. Consumers dec after use.

## Design Principles

1. **Uniform ownership at the yield boundary** (Swift model). Every yielded element is owned. No per-iterator flags, no chain analysis, no compile-time ownership inference. Make it correct first; AIMS can optimize later.

2. **SSOT for element cleanup.** `elem_dec_fn` is the single mechanism for element cleanup. Consumers receive it as a parameter (not stored in headers, not inferred from state). The codegen generates it via the existing `get_or_generate_elem_dec_fn()`.

3. **Minimal API surface change.** Add `elem_dec_fn` to consumer signatures. Add `elem_inc_fn` to source constructors. Do NOT redesign IterState or the adapter chain.

## Section Dependency Graph

```
01 (Sources: inc on yield)
  |
02 (Adapters: filter/skip dec discards) ──depends on── 01
  |
03 (Consumers: dec after use) ──depends on── 01
  |
04 (Codegen: pass elem_inc/dec_fn) ──depends on── 01, 02, 03
  |
05 (AIMS: remove borrowed_defs suppression) ──depends on── 04
  |
06 (Verification: full matrix testing) ──depends on── all above
```

**Implementation Sequence:**
- Phase 0 (Section 01): Runtime source changes — `next_list` and `next_str` call `elem_inc_fn`
- Phase 1 (Sections 02-03): Runtime adapter + consumer changes — parallel (independent)
- Phase 2 (Section 04): Codegen — pass new parameters through LLVM emission
- Phase 3 (Section 05): AIMS pipeline — remove/rework borrowed element suppression
- Phase 4 (Section 06): Verification matrix — consumer x adapter x type

## Estimated Effort

| Section | Est. Lines | Complexity | Depends On |
|---------|-----------|------------|------------|
| 01 Sources: Inc on Yield | ~80 | Medium | — |
| 02 Adapters: Dec Discards | ~60 | Medium | 01 |
| 03 Consumers: Dec After Use | ~150 | Medium | 01 |
| 04 Codegen: Param Plumbing | ~200 | High | 01, 02, 03 |
| 05 AIMS: Owned Elements | ~100 | High | 04 |
| 06 Verification Matrix | ~300 | Medium | all |
| **Total new** | **~890** | | |

## Known Bugs (Pre-existing)

| Bug | Root Cause | Fix Location | Status |
|-----|-----------|-------------|--------|
| BUG-05-003 | `ori_iter_join` direct-string path leaks owned elements from adapters | Section 03 | Not Started |
| BUG-05-002 | `ori_iter_join` trampoline path leaks heap OriStr | Fixed (2296d3f2) | Fixed |
| Collect double-inc | `collect` incs already-owned adapter elements to RC=2 | Section 03 | Not Started |
| Filter/skip silent discard | Rejected elements discarded without cleanup | Section 02 | Not Started |
| find/last dangling | Returned elements may dangle after iterator Drop | Section 03 | Not Started |
| rev/cycle storage bugs | Collected elements in Vec without proper RC | Section 02 | Not Started |

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Sources: Inc on Yield | `section-01-sources.md` | Not Started |
| 02 | Adapters: Dec on Discard | `section-02-adapters.md` | Not Started |
| 03 | Consumers: Dec After Use | `section-03-consumers.md` | Not Started |
| 04 | Codegen: Parameter Plumbing | `section-04-codegen.md` | Not Started |
| 05 | AIMS: Owned Element Contract | `section-05-aims.md` | Not Started |
| 06 | Verification Matrix | `section-06-verification.md` | Not Started |
