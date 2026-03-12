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
**File:** `section-01-lattice.md` | **Status:** In Progress

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
SizeClass, ReuseCtorKind, chain height, iteration_limit, finite height
Perceus, lambda_1, FIP, FP2, Chirimar, Atkey, QTT, OxCaml
lattice/mod.rs, lattice/dimensions.rs, transfer/mod.rs, aims_state, core dimensions, auxiliary dimensions
```

---

### Section 02: Intraprocedural Analysis
**File:** `section-02-intraprocedural.md` | **Status:** In Progress

```
backward dataflow, forward dataflow, per-function, transfer functions
ArcInstr, ArcBlock, ArcTerminator, basic block, CFG traversal
liveness, refined liveness, kill set, gen set, worklist
analyze_function, compute_block_entry_state, AimsStateMap
AimsEvent, ContextOpen, ContextClose, ReusableAllocation
LocalAllocCandidate, FipGate, sparse event table
seq_add, alt_join, demand semiring, cardinality, GHC demand analysis
InvokeEdgeState, normal edge, unwind edge, invoke cleanup
ContextRegion, context_regions, reset_changed, convergence, non-convergence safety net
validation corpus, hand-traced tests
```

---

### Section 03: Interprocedural Analysis
**File:** `section-03-interprocedural.md` | **Status:** Complete

```
SCC, Tarjan, call graph, fixed-point, monotonic
borrow inference, ownership signature, AnnotatedSig
MemoryContract, ParamContract, ReturnContract, AimsSig (alias)
EffectSummary, ContextBehavior, FipContract
interprocedural uniqueness, UniquenessSummary
convergence, iteration count, widening
Lean 4, infer_borrows_scc, collect_O, analyze_program, extract_contract, FP2
BuiltinOwnershipSets, preserves_freshness, all_borrowed, all_owned
```

---

### Section 04: RC Emission
**File:** `section-04-rc-emission.md` | **Status:** Complete

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
**File:** `section-05-reuse-emission.md` | **Status:** Complete

```
reset, reuse, reuse token, reuse credit, constructor reuse
drop-guided reuse, resurrection hypothesis, allocation pairing
reset_reuse, expand_reuse, IsShared, Set, SetTag
FIP, FipContract, FipGate, certified, conditional
FBIP, check_fbip_enforcement, is_auto_fbip, diagnostic
reuse specialization, field skip
ReusePlanner, DeathEvent, AllocEvent, SizeClass, planner.rs
Lean 4, Koka, Perceus, FP2, frame-limited
CollectionReuse, cross-block reuse, dominator tree, post-dominator tree
SuppressedDeath, EmitReuseResult, FipGateRecord, ProjMap
```

---

### Section 06: Pipeline Integration
**File:** `section-06-pipeline.md` | **Status:** Complete

```
pipeline/mod.rs, run_arc_pipeline, run_arc_pipeline_all, run_uniqueness_analysis
pass ordering, single traversal, analysis-emission separation
ArcFunction, var_reprs, cow_annotations, drop_hints
ArcParam, arg_ownership, apply_borrows, annotate_arg_ownership
tail_call, block_merge, verify, fbip
old pass removal, feature flag, gradual migration, aims feature
feature propagation, ori_llvm, oric, compatibility wrapper
lib.rs, re-exports, cache feature, serde
run_aims_pipeline, AimsPipelineConfig, apply_ownership, aims_pipeline.rs
normalize_function, opportunity creation, three-stage
Stage 1A, Stage 1B, Stage 1C, Stage 1D, shadow analysis, ShadowComparisonReport
DimensionResult, FunctionComparison, RcOpCount, aims-shadow
```

---

### Section 07: Advanced Optimizations
**File:** `section-07-advanced.md` | **Status:** In Progress

```
immortal objects, ImmortalSet, MAX_REFCOUNT, ori_str_empty, SSO
static coalescing, coalesce, RcInc merge, inc-dec cancellation, peephole
COW-aware borrowing, uniqueness-preserving borrow, BorrowSource, disjoint field
demand-driven RC, absent parameter, Absent cardinality, RC-skip
cross-optimization synergy, cross-dimensional, StaticUnique upgrade
whole-program mutability, Morphic, Lobster
SCC-frozen cyclic RC, Parkinson, concurrent RC, CIRC
representation optimization, bit-stealing, Elsman, RcStrategy
```

---

### Section 08: Verification & Validation
**File:** `section-08-verification.md` | **Status:** In Progress

```
behavioral equivalence, dual execution, dual-exec-verify, regression
RC operation count, allocation count, elimination rate, RcOpCount
aims-compare.sh, golden corpus, corpus freeze policy
FIP certification coverage, FBIP achieved, missed reuse
ORI_DUMP_AFTER_ARC, arc_dump, arc_dot, GraphViz
test matrix, spec tests, AOT tests, valgrind, ORI_CHECK_LEAKS
performance validation, compilation speed, codegen quality, hyperfine
same-compiler comparison, Exploring Perceus for OCaml, evaluation doctrine
confounding-variable isolation, compile-time breakdown
```

---

### Section 09: Dimensional Fusion
**File:** `section-09-dimensional-fusion.md` | **Status:** Not Started

```
dimensional fusion, transfer fusion, cross-talk, active dimensions
locality activation, effect activation, shape activation
enriched canonicalize, cross-dimension invariants, rules 4-8
sequencing algebra extension, seq_add, alt_join, locality seq
convergence feedback, multi-round canonicalize, cross-dim tightening
BlockLocal+Owned+Once→Unique, pure callee preserves uniqueness
one team, one system, integrated analysis, not separate passes
COW as view, FIP as view, reuse as view, output views
backward analysis semantics, locality backward, effect accumulation
locality_bound, ParamContract sync, canonicalize soundness guard
Rule 4 soundness, FIP classification in extract_contract, function-level EffectSummary
interprocedural demand propagation, callee cardinality tightening
error handling, fallback strategy, non-convergence, rollback, regression response
sync points, FipContract::Bounded, MemoryContract.is_fbip, AllocCreditBalance
```

---

### Section 10: Unified Realization
**File:** `section-10-unified-realization.md` | **Status:** Not Started

```
unified realization, realize(), two-phase emission
realize_rc_reuse, realize_annotations, Phase 1, Phase 2
decide(), InstructionDecisions, AnnotationDecisions
decide_annotations, per-instruction decision, per-variable annotation
RealizationResult, output views, COW view, reuse view, FipEvidence
merge emit_rc, merge emit_reuse, merge cow, merge drop_hints
edge cleanup, trampoline blocks, post-merge, ArcVarId-keyed lookups
arg_ownership disposition, Option A/B/C, emit_arg_ownership
pipeline steps, pre-merge, post-merge, aims_pipeline.rs
migration strategy, rollback, use_realize flag, output equivalence
sync points, AnnotationSiteContext, DecisionContext, SynergyMetrics
old code deletion, emit_rc deletion, emit_reuse deletion
```

---

### Section 11: Integration Verification
**File:** `section-11-integration-verification.md` | **Status:** Not Started

```
integration verification, cross-dimension test programs
synergy metrics, SynergyMetrics, multi_dim_rc_decisions
regression guards, per-rule regression, golden corpus gate
tests/aims/synergy/, block_local_unique, pure_callee_preserves
seven_dimensions, collection_buffer_unique, effect_fip_natural
compilation speed gate, quantitative proof
ArcIrBuilder test pattern, manual ArcFunction construction
Rule 4-8 unit tests, fire/no-fire, canonicalize regression
SynergyMetrics Phase 1, SynergyMetrics Phase 2, disabled_canonicalize_rules
Option A/B/C test methodology, end-to-end RC count assertion
Stage 2 exit gate, aims-shadow retirement, legacy deletion
test file locations, Ori program verification
```

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| — | Overview & Architecture | `00-overview.md` |
| 01 | Unified Lattice Design | `section-01-lattice.md` |
| 02 | Intraprocedural Analysis | `section-02-intraprocedural.md` |
| 03 | Interprocedural Analysis | `section-03-interprocedural.md` |
| 04 | RC Emission | `section-04-rc-emission.md` |
| 05 | Reuse Emission | `section-05-reuse-emission.md` |
| 06 | Pipeline Integration | `section-06-pipeline.md` |
| 07 | Advanced Optimizations | `section-07-advanced.md` |
| 08 | Verification & Validation | `section-08-verification.md` |
| 09 | Dimensional Fusion | `section-09-dimensional-fusion.md` |
| 10 | Unified Realization | `section-10-unified-realization.md` |
| 11 | Integration Verification | `section-11-integration-verification.md` |

## Performance Validation

Use `/benchmark short` after modifying hot paths.

**When to benchmark:** Sections 02, 03, 06, 09 (compilation speed); Sections 08, 11 (codegen quality, end-to-end)
**Skip benchmarks for:** Section 01 (data structures only)
