---
reroute: true
name: "Locality SSOT"
full_name: "Locality Representation Unification — SSOT for Escape Classification"
status: queued
order: 0
---

# Locality Representation Unification Index

> **Maintenance Notice:** Update this index when adding/modifying sections.
>
> **Reroute priority note:** This plan is queued at `order: 0` (highest queue priority)
> because it is a **hard prerequisite** to `plans/repr-opt/`'s sections §08 (escape
> analysis), §09 (ARC header compression), and §10 (thread-local non-atomic ARC). Those
> three sections currently plan to define parallel `EscapeState` and `ThreadLocality`
> enums; this plan unifies escape-scope classification into `ori_arc::Locality` first
> so the repr-opt sections consume the unified type instead. Section 04 of this plan
> performs the cross-plan text coordination (re-read-before-edit protocol) in the
> `plans/repr-opt/` corpus when executed. `repr-opt` is currently `status: active,
> order: 1` — this plan should be promoted to `active` ahead of repr-opt §08 execution,
> OR repr-opt should pause at §07 and yield to this plan. The exact handoff is a
> scheduling decision for the developer when repr-opt approaches §08.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 00: Hygiene Foundation
**File:** `section-00-hygiene-foundation.md` | **Status:** Not Started

```
hygiene foundation, file split, 500-line limit, BLOAT
lattice/mod.rs split (552→≤80), transfer/mod.rs split (524→≤80)
interprocedural/extract.rs split (517→≤250 leaf-to-directory promotion)
extract/mod.rs, extract/consumed_params.rs, extract/return_info.rs
intraprocedural/state_map.rs split (646→≤300 leaf-to-directory + multiple impl blocks)
state_map/mod.rs, state_map/events.rs, state_map/cross_dim.rs, state_map/borrow_provenance.rs, state_map/invoke_shape.rs, state_map/effects_fip.rs
interprocedural/mod.rs split (536→≤300 with scc_loop.rs extracted)
analyze_scc_fixpoint extraction, tighten_uniqueness_from_callers extraction
submodule structure, dispatch hub, leaf implements pattern
public API surface preservation, private mod, pub use re-exports
cargo public-api not installed, grep fallback primary, future upgrade
stale annotation rewrite, Section 09.2, Section 09.3, Section 09.5
plan-annotations.sh, DRIFT category, impl-hygiene.md
Effect Activation, Shape Activation, Convergence Feedback
99 stale references across AIMS subtree (~27 post-split files)
no semantic change, structural cleanup, hygiene-only
architectural seams, not mechanical chunking
```

---

### Section 01: Representation Decision
**File:** `section-01-representation-decision.md` | **Status:** Not Started

```
representation decision, decision log, no code
chain ordering, BlockLocal, FunctionLocal, ArgEscaping, HeapEscaping, Unknown
Locality enum extension, single new variant, additive change
Rule 6 decision, ArgEscaping + Unique stays Unique
EscapeInfo API, FxHashMap<ArcVarId, Locality>, monotone writer
join_escape_scope, escape_scope, escapes, is_non_escaping
soundness conditions, monotonicity, ownership preservation
no heap persistence, multi-callee join, lifetime bound
producer obligation, consumer responsibility
Pass 4 prior art citation, Go leakCallee, Lean 4 borrow inference
golang escape.go, leaks.go, calleeLoc, attrEscapes
Lean Borrow.lean, initBorrow, isPossibleRef
Swift escape destinations, ownership conventions, @guaranteed
Roc value semantics, no escape analysis
```

---

### Section 02: ori_arc Implementation
**File:** `section-02-ori-arc-implementation.md` | **Status:** Not Started

```
ori_arc implementation, ArgEscaping variant, dimensions.rs
Locality enum, lattice/dimensions.rs:165-176, discriminant order
PartialOrd, Ord, Hash, derive preservation
canonicalize Rules 4 6 8, semantic preservation
Rule 4 BlockLocal Owned Once Unique, Rule 6 HeapEscaping MaybeShared
Rule 8 Borrowed FunctionLocal, cross-dimension chain
CHAIN_HEIGHT 15 to 16, iteration_limit recompute
AimsState TOP FRESH SCALAR OPTIMISTIC, BOTTOM
Locality::join, max semantics
ParamContract::may_escape conversion, derived method
LEAK scattered-knowledge, SSOT enforcement
contract/mod.rs CONSERVATIVE OPTIMISTIC, parallel state removal
ParamContract::join, may_escape OR removal
ReturnContract::locality, drift check
predicate audit, 13 sites across 7 files, no exhaustive matches
matches predicate vs exhaustive match, semantic per-site decision
intraprocedural/block.rs:97 cross-block widening, block.rs:155 return widening
intraprocedural/effects.rs:38 order predicate, automatic correctness
intraprocedural/state_map.rs:429 order predicates
intraprocedural/post_convergence.rs:95 matches predicate is_local_alloc_eligible
transfer/state_helpers.rs (post-split) capture_state_update locality widening
interprocedural/extract/mod.rs (post-Section-00.4b split) ParamContract construction PRODUCTION
builtins/mod.rs ParamContract literals 286, 297 PRODUCTION
verify/tests.rs locality assertions, ParamContract literals 486, 498
realize/tests.rs locality assertion (line 1091)
all_locality helper at tests.rs:26, extends 4 to 5 variants (no pre-existing bug)
representative_states locality sample at tests.rs:68 (2 → 3 variants)
ParamContract may_escape field, 7 literal sites struct construction
test extension, commutativity 5x5, associativity 5x5x5
Lean 4 inversion comment, dimensions.rs doc update
Pass 4 citation, Borrow.lean:58-60
ParamContract::may_escape() derivation matrix, contract/tests.rs, 5 per-variant + matrix completeness + negative pin
return widening producer-site soundness pin, intraprocedural/tests.rs, block.rs:155, condition 4 enforcement
```

---

### Section 03: ori_repr EscapeInfo Storage
**File:** `section-03-ori-repr-escape-info.md` | **Status:** Not Started

```
ori_repr escape info, EscapeInfo storage, per-function map
escape/mod.rs replacement, placeholder removal, FxHashMap
ArcVarId Locality map, var_escape, monotone writer
escape_scope, escapes, is_non_escaping, join_escape_scope
boolean helpers, query API alignment
ReprPlan::escapes() body replacement, plan/query.rs:111
hardcoded true removal, escape_info lookup
rc_strategy() pattern, query.rs:124-134, mirror
Default::default(), conservative default, behavioral parity
cross-crate import, ori_arc::aims::lattice::Locality
ori_arc::ir::ArcVarId, plan.rs:21 existing import
EscapeInfo tests, query body tests
behavioral parity test, hardcode replication
```

---

### Section 04: Plan Corpus Coordination
**File:** `section-04-plan-corpus-coordination.md` | **Status:** Not Started

```
plan corpus coordination, cross-plan updates, depends_on
repr-opt section-08-escape-analysis.md, EscapeState removal
section-08 §08.1 task list rewrite, consume Locality
section-08 soundness conditions subsection, conditions 2 and 4
ownership preservation enforcement, no heap persistence enforcement
producer site assertions, block.rs:97 block.rs:155
repr-opt section-09-arc-header.md, compute_sharing_bound update
EscapeState::NoEscape replacement, EscapeInfo::is_non_escaping
repr-opt section-10-thread-local-arc.md, RcStrategy confirmation
NonAtomic vs Atomic, no parallel ThreadLocality enum
00-overview.md:191-192 update, unified Locality references
index.md keyword cluster cleanup, EscapeState ThreadLocality removal
cross-reference resolution, /review-plan repr-opt
```

---

### Section 05: Verification & Documentation
**File:** `section-05-verification.md` | **Status:** Not Started

```
verification documentation, test matrix
5 variants times 3 canonicalize rules, exhaustive coverage
self-verifying matrix completeness, canonicalize idempotence
soundness pin test, ArgEscape Unique stays Unique
Rule 6 negative test, behavioral pin
cross-crate behavioral test, EscapeInfo round-trip
ori_repr ori_arc Locality flow
plan annotation cleanup scan, plan-annotations.sh
0 stale Section 09 references in AIMS files
lattice/mod.rs module doc update, unified representation
5 soundness conditions documentation
Go leakCallee citation, Lean 4 borrow citation
test-all.sh green, clippy-all.sh green
tpr-review final, impl-hygiene-review final
improve-tooling retrospective, diagnostic gaps
```

---

## Quick Reference

| ID | Title | File | Status |
|---|---|---|---|
| 00 | Hygiene Foundation | `section-00-hygiene-foundation.md` | Not Started |
| 01 | Representation Decision | `section-01-representation-decision.md` | Not Started |
| 02 | ori_arc Implementation | `section-02-ori-arc-implementation.md` | Not Started |
| 03 | ori_repr EscapeInfo Storage | `section-03-ori-repr-escape-info.md` | Not Started |
| 04 | Plan Corpus Coordination | `section-04-plan-corpus-coordination.md` | Not Started |
| 05 | Verification & Documentation | `section-05-verification.md` | Not Started |
