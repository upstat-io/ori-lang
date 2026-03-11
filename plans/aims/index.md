---
reroute: true
name: "AIMS"
full_name: "ARC Intelligent Memory System"
status: active
order: 1
---

# AIMS — ARC Intelligent Memory System

> **Maintenance Notice:** Update this index when adding/modifying sections.

> **No Deferrals.** Every checkbox in every section must be implemented. Do not
> mark items as deferred, skip items, or move items to later stages. Work each
> section's items in order until all checkboxes are checked.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: Unified Lattice Design
**File:** `section-01-lattice.md` | **Status:** Not Started

```
lattice, AimsState, substructural, ownership, uniqueness, cardinality
AccessClass, Consumption, Borrowed, Owned, Dead, Linear, Affine, Unrestricted
BorrowSource, borrow provenance, canonicalize, stratified reduced product
Locality, ShapeClass, EffectClass, modal fact domain, product lattice
semiring, join, meet, transfer function, abstract interpretation
Unique, MaybeShared, Shared, Absent, Once, Many, Fresh
BlockLocal, FunctionLocal, HeapEscaping, Unknown, NonReusable, ReusableCtor
CollectionBuffer, ContextHole, Pure, MayAlloc, MayShare, MayThrow
ArcClass, PossibleRef, DefiniteRef, from_arc_class, FRESH, SCALAR
Perceus, lambda_1, FIP, FP2, Chirimar, Atkey, QTT, OxCaml
lattice.rs, transfer.rs, aims_state, core dimensions, auxiliary dimensions
```

---

### Section 02: Intraprocedural Analysis
**File:** `section-02-intraprocedural.md` | **Status:** Not Started

```
backward dataflow, forward dataflow, per-function, transfer functions
ArcInstr, ArcBlock, ArcTerminator, basic block, CFG traversal
liveness, refined liveness, kill set, gen set, worklist
analyze_function, compute_block_entry_state, AimsStateMap
AimsEvent, ContextOpen, ContextClose, ReusableAllocation
LocalAllocCandidate, FipGate, sparse event table
seq_add, alt_join, demand semiring, cardinality, GHC demand analysis
TerminatorEdgeState, normal edge, unwind edge, invoke cleanup
validation corpus, hand-traced tests
```

---

### Section 03: Interprocedural Analysis
**File:** `section-03-interprocedural.md` | **Status:** Not Started

```
SCC, Tarjan, call graph, fixed-point, monotonic
borrow inference, ownership signature, AnnotatedSig
MemoryContract, ParamContract, ReturnContract, AimsSig (alias)
EffectSummary, ContextBehavior, FipContract
interprocedural uniqueness, UniquenessSummary
convergence, iteration count, widening
Lean 4, infer_borrows_scc, collect_O, analyze_program, FP2
BuiltinOwnershipSets, preserves_freshness, all_borrowed, all_owned
```

---

### Section 04: RC Emission
**File:** `section-04-rc-emission.md` | **Status:** Not Started

```
RcInc, RcDec, emit, insertion, placement
last use, dead variable, live set, consumption
rc_insert, annotate_arg_ownership, drop insertion
arg_ownership, ArgOwnership, edge_cleanup, critical edge
Perceus dup/drop, linear resource, structural rules
invoke cleanup, unwind edge, landingpad
CollectionReuse, cow_annotations, drop_hints
CowMode, StaticUnique, StaticShared, Dynamic
emit_rc_ops, emit_arg_ownership, RcStrategy, BorrowSource
insert_edge_cleanup, trampoline blocks, var_reprs, ValueRepr
```

---

### Section 05: Reuse Emission
**File:** `section-05-reuse-emission.md` | **Status:** Not Started

```
reset, reuse, reuse token, reuse credit, constructor reuse
drop-guided reuse, resurrection hypothesis, allocation pairing
reset_reuse, expand_reuse, IsShared, Set, SetTag
FIP, FipContract, FipGate, certified, conditional
FBIP, check_fbip_enforcement, is_auto_fbip, diagnostic
reuse specialization, field skip
ReusePlanner, DeathEvent, AllocEvent, SizeClass
Lean 4, Koka, Perceus, FP2, frame-limited
CollectionReuse, cross-block reuse, dominator tree, post-dominator tree
```

---

### Section 06: Pipeline Integration
**File:** `section-06-pipeline.md` | **Status:** Not Started

```
pipeline.rs, run_arc_pipeline, run_arc_pipeline_all, run_uniqueness_analysis
pass ordering, single traversal, analysis-emission separation
ArcFunction, var_reprs, cow_annotations, drop_hints
ArcParam, arg_ownership, apply_borrows, annotate_arg_ownership
tail_call, block_merge, verify, fbip
old pass removal, feature flag, gradual migration, aims feature
feature propagation, ori_llvm, oric, compatibility wrapper
lib.rs, re-exports, cache feature, serde
run_aims_pipeline, AimsPipelineConfig, apply_ownership
normalize_function, opportunity creation, three-stage
Stage 1A, Stage 1B, Stage 1C, Stage 1D, shadow analysis, ShadowComparisonReport
```

---

### Section 07: Advanced Optimizations
**File:** `section-07-advanced.md` | **Status:** Not Started

```
immortal objects, static coalescing
whole-program mutability, Morphic, Lobster
drop specialization, reuse specialization, field skip
demand-driven RC, one-shot closure, absent parameter
biased RC, dynamic atomicity, thread-local
SCC-frozen cyclic RC, Parkinson, concurrent RC, CIRC
representation optimization, bit-stealing, Elsman
```

---

### Section 08: Verification & Validation
**File:** `section-08-verification.md` | **Status:** Not Started

```
behavioral equivalence, dual execution, regression
RC operation count, allocation count, elimination rate, comparison
FIP certification coverage, FBIP achieved, missed reuse
ORI_DUMP_AFTER_ARC, arc_dump, arc_dot, GraphViz
test matrix, spec tests, AOT tests, valgrind
performance validation, compilation speed, codegen quality
same-compiler comparison, Exploring Perceus for OCaml, evaluation doctrine
compile-time breakdown, normalization overhead
```

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| — | Overview & Architecture | `00-overview.md` |
| — | Research Integration (historical) | `improvements.md` |
| — | Risk Solutions (historical) | `solutions.md` |
| 01 | Unified Lattice Design | `section-01-lattice.md` |
| 02 | Intraprocedural Analysis | `section-02-intraprocedural.md` |
| 03 | Interprocedural Analysis | `section-03-interprocedural.md` |
| 04 | RC Emission | `section-04-rc-emission.md` |
| 05 | Reuse Emission | `section-05-reuse-emission.md` |
| 06 | Pipeline Integration | `section-06-pipeline.md` |
| 07 | Advanced Optimizations | `section-07-advanced.md` |
| 08 | Verification & Validation | `section-08-verification.md` |

## Performance Validation

Use `/benchmark short` after modifying hot paths.

**When to benchmark:** Sections 02, 03, 06 (compilation speed); Section 08 (codegen quality, end-to-end)
**Skip benchmarks for:** Section 01 (data structures only), Section 07 (post-integration stretch goals)
