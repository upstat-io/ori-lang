# Clang ARC Lessons Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: Compile-Time ARC Statistics
**File:** `section-01-statistics.md` | **Status:** Not Started

```
SynergyMetrics, rc_ops_before, rc_ops_after, coalesce_reduction
barrier_flush_count, STATISTIC, tracing::info, compile-time counters
aims_pipeline.rs, metrics.rs, realize/mod.rs
LLVM ObjCARCOpts.cpp NumRRs NumNoops NumRetainsBeforeOpt
```

---

### Section 02: Effect-Aware Coalescing Barriers
**File:** `section-02-barriers.md` | **Status:** Not Started

```
coalesce_block_rc, flush_all, flush_selective, barrier
MemoryContract, ParamContract, AccessClass, Consumption
Apply, ApplyIndirect, callee_may_observe_rc
Borrowed parameter, Owned transfer, call barrier
coalesce/mod.rs, contract/mod.rs, arg_ownership.rs
Lean4 Borrow.lean, Swift RCStateTransition.cpp
DependencyAnalysis.cpp, CanAlterRefCount, CanDecrementRefCount
```

---

### Section 03: KnownSafe Nested Pair Elimination
**File:** `section-03-knownsafe.md` | **Status:** Not Started

```
KnownSafe, nested pair, retain release elimination
physical refcount positive, monotonic flag, KnownPositiveRefCount
RcInfo, RefcountState, Positive Unknown Decremented
PtrState.h, ObjCARCOpts.cpp, MatchWithRetain, MatchWithRelease
Swift RefCountState.h, ARCSequenceOpts.cpp
post-emission analysis, refcount bracketing
```

---

### Section 04: Late COW Compound Contraction
**File:** `section-04-cow-contraction.md` | **Status:** Not Started

```
COW contraction, compound op, ori_cow_mutate, CowMutate
IsShared, branch, clone, mutate, storeStrong
CowMode, StaticUnique, Dynamic, StaticShared
decide_annotations, is_borrow_disjoint_from_siblings
ObjCARCContract.cpp, tryToContractReleaseIntoStoreStrong
arc_emitter, codegen, runtime intrinsic evaluation
ir/instr.rs, emit_rc/cow_contract.rs, instr_dispatch.rs
```

---

### Section 05: PRE-Style Global RC Code Motion
**File:** `section-05-rc-motion.md` | **Status:** Not Started

```
PRE, partial redundancy elimination, global code motion
bidirectional dataflow, bottom-up, top-down, path counting
VisitBottomUp, VisitTopDown, PairUpRetainsAndReleases
BBState, TopDownPathCount, BottomUpPathCount, CFGHazardAfflicted
ReverseInsertPts, retain release placement
region-aware, loop forest, summarization
Swift ARCSequenceOpts, GlobalARCSequenceDataflow
```

---

### Section 06: Verification
**File:** `section-06-verification.md` | **Status:** Not Started

```
test matrix, behavioral equivalence, dual-exec parity
code journey, regression, debug release parity
ORI_CHECK_LEAKS, ORI_TRACE_RC, rc-stats.sh
test-all.sh, clippy-all.sh, valgrind
```

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| 01 | Compile-Time ARC Statistics | `section-01-statistics.md` |
| 02 | Effect-Aware Coalescing Barriers | `section-02-barriers.md` |
| 03 | KnownSafe Nested Pair Elimination | `section-03-knownsafe.md` |
| 04 | Late COW Compound Contraction | `section-04-cow-contraction.md` |
| 05 | PRE-Style Global RC Code Motion | `section-05-rc-motion.md` |
| 06 | Verification | `section-06-verification.md` |
