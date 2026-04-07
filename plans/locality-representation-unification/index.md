# Locality Representation Unification Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

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
lattice/mod.rs split, transfer/mod.rs split, submodule structure
stale annotation rewrite, Section 09.2, Section 09.3, Section 09.5
plan-annotations.sh, DRIFT category, impl-hygiene.md
Effect Activation, Shape Activation, Convergence Feedback
17 stale references, 5 affected AIMS files
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
consumer migration, 8 non-test files
intraprocedural/block.rs:97 cross-block widening, block.rs:155 return widening
intraprocedural/effects.rs:38 order predicate, automatic correctness
intraprocedural/state_map.rs:429 order predicates
transfer/mod.rs producer sites, transfer_project transfer_apply_conservative
transfer_collection_reuse, capture_state_update
interprocedural/extract.rs:84 ParamContract construction
builtins/mod.rs locality bound writes
verify/tests.rs locality assertions
realize/decide.rs locality reads
all_locality helper at tests.rs:26, pre-existing test gap fix
test extension, commutativity 5x5, associativity 5x5x5
Lean 4 inversion comment, dimensions.rs doc update
Pass 4 citation, Borrow.lean:58-60
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

| ID | Title | File |
|---|---|---|
| 00 | Hygiene Foundation | `section-00-hygiene-foundation.md` |
| 01 | Representation Decision | `section-01-representation-decision.md` |
| 02 | ori_arc Implementation | `section-02-ori-arc-implementation.md` |
| 03 | ori_repr EscapeInfo Storage | `section-03-ori-repr-escape-info.md` |
| 04 | Plan Corpus Coordination | `section-04-plan-corpus-coordination.md` |
| 05 | Verification & Documentation | `section-05-verification.md` |
