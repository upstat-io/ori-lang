---
reroute: true
name: "AIMS"
full_name: "ARC Intelligent Memory System"
status: active
order: 1
---

# AIMS — ARC Intelligent Memory System

> **Maintenance Notice:** Update this index when adding/modifying sections.

> **Completion Rule.** A section is complete only when implementation exists,
> invariants are enforced, verification exists, and downstream consumers use
> the same truths. See `00-overview.md` §7 for the full rule.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: Unified Lattice Design
**File:** `section-01-lattice.md` | **Status:** Complete

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
**File:** `section-02-intraprocedural.md` | **Status:** Complete

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
**File:** `section-06-pipeline.md` | **Status:** Complete (legacy dead code deleted, live modules retained)

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
**File:** `section-07-advanced.md` | **Status:** Complete

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
**File:** `section-08-verification.md` | **Status:** In Progress (cross-system interaction matrix 08.5a not started)

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
Matrix H, cross-system interaction, subsystem boundary testing
TRMC x RC, TRMC x reuse, TRMC x COW, TRMC x FIP, TRMC x contracts
RC x tail_call, RC x block_merge, reuse x drop_hints, COW x FIP
three-layer assertion strategy, ARC unit, AOT behavioral, Valgrind leak
```

---

### Section 09: Dimensional Fusion
**File:** `section-09-dimensional-fusion.md` | **Status:** Complete

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
**File:** `section-10-unified-realization.md` | **Status:** Complete

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
**File:** `section-11-integration-verification.md` | **Status:** Incomplete

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

### Section 12: FIP Proof Obligations & Enforcement
**File:** `section-12-fip-enforcement.md` | **Status:** Complete

```
FIP proof obligations, FP2 Theorem 2, allocation balance, deallocation balance
may_deallocate, EffectSummary, constant stack, has_unbounded_stack
FipContract::Certified, FipContract::Bounded, FipContract::Conditional
FIP enforcement verifier, verify_fip_contract, FipVerificationError
CertifiedButHasMissedReuses, CertifiedButUnboundedStack, BoundedExceeded
FipEvidence, missed_reuses, contract/emission mismatch
extract_contract, is_fbip, may_allocate, post-emission update
tail-call rewriting, syntactic tail position, self-recursive SCC
is_in_tail_position, tail_call/mod.rs, pre-emission tail position check
stale documentation, Stage 1 banner, contract/mod.rs
aims/verify/mod.rs, aims/verify/fip.rs, pub mod verify
accumulate_effect, state_map.rs, EffectSummary Default derive
struct_excessive_bools, clippy reason, 6 independent effect flags
Koka CheckFBIP, Lean 4 RC.lean, FP2 Theorem 2
pipeline/aims_pipeline.rs, post-emission may_deallocate update, step 5a
```

---

### Section 13: TRMC Realization & Soundness
**File:** `section-13-trmc-realization.md` | **Status:** In Progress (structural bugs fixed, behavioral test matrix 13.8 not started)

```
TRMC, tail recursion modulo context, modulo cons, constructor context
ContextBehavior, preserves_context, consumes_hole, requires_unique_context
may_resume_nonlinearly, interprocedural inference, extract_contract
ContextBehavior Default, manual Default impl, derive removal
soundness gate, may_share, per-variable uniqueness, Lemma 2
unique linear chain, effect purity, non-linear resumption
fixpoint iteration edge case, first SCC iteration, local_sigs
effect_summary, state_map.effect_summary(), post-convergence may_share
lifting pre-pass, A-normal form, lift_constructor_args, var_types extension
4-equation algorithm, base, tail, tlet, tmatch, Figure 2
Minamide tuple, context composition, context application
in-place transform, optional context parameter, auxiliary function
rewrite_trmc, normalize/rewrite.rs, normalize/lift.rs, normalize/verify.rs
TrmcContext, TrmcVerificationError, NonLinearContext, NonUniqueContext
context laws, appctx, appcomp, post-rewrite verification, verify_trmc_soundness
func.clone(), rollback mechanism, verification failure restore
ContextOpen, ContextClose, AimsEvent, event consumption
NormalizationResult, was_transformed, context_regions, rollback
&mut ArcFunction, normalize_function signature change, Option<&MemoryContract>
detect_trmc_candidates, populate_context_events, detect_context_regions
pipeline re-analysis, normalize_function, aims_pipeline.rs
compute_var_reprs re-run, detect_immortals re-run, idempotent rewrite
Leijen Lorenzen JFP 2025, FIPTree PLDI 2024, Koka CTail
BUG FIXED: recursive argument threading, BUG PARTIAL: stale contracts, BUG FIXED: helper block params
BUG FIXED: may_share gate blocks all candidates, BUG FIXED: uniqueness verification stubbed
Matrix D, Matrix E, Matrix F, Matrix G, behavioral test matrix
TRMC x RC emission, TRMC x reuse, TRMC x COW, TRMC x FIP, TRMC x contracts
```

---

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| — | Overview | `00-overview.md` | — |
| 01 | Unified Lattice Design | `section-01-lattice.md` | Complete |
| 02 | Intraprocedural Analysis | `section-02-intraprocedural.md` | Complete |
| 03 | Interprocedural Analysis | `section-03-interprocedural.md` | Complete |
| 04 | RC Emission | `section-04-rc-emission.md` | Complete (superseded by 10) |
| 05 | Reuse Emission | `section-05-reuse-emission.md` | Complete (superseded by 10) |
| 06 | Pipeline Integration | `section-06-pipeline.md` | Complete |
| 07 | Advanced Optimizations | `section-07-advanced.md` | Complete |
| 08 | Verification & Validation | `section-08-verification.md` | In Progress (08.5a not started) |
| 09 | Dimensional Fusion | `section-09-dimensional-fusion.md` | Complete |
| 10 | Unified Realization | `section-10-unified-realization.md` | Complete |
| 11 | Integration Verification | `section-11-integration-verification.md` | Incomplete |
| 12 | FIP Proof Obligations | `section-12-fip-enforcement.md` | Complete |
| 13 | TRMC Realization | `section-13-trmc-realization.md` | In Progress (structural fixes done, behavioral tests pending) |

## Performance Validation

Use `/benchmark short` after modifying hot paths.

**When to benchmark:** Sections 02, 03, 06, 09, 13 (compilation speed); Sections 08, 11 (codegen quality, end-to-end)
**Skip benchmarks for:** Section 01 (data structures only), Section 12 (verification-only, no hot paths)
