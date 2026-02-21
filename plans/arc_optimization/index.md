# ARC Optimization Plan Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

> **Design Reference:** `plans/dpr_arc-optimization_02212026.md` — Design Pattern Review with prior art analysis (Swift, Lean 4, Koka) and proposed best-of-breed design.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: LLVM Codegen Completeness
**File:** `section-01-codegen.md` | **Status:** Not Started

```
atomic refcount, ori_rc_inc, ori_rc_dec, AtomicI64
fetch_add, fetch_sub, Relaxed, Release, Acquire
drop function, DropInfo, DropKind, arc_emitter
Trivial, Fields, Enum, Collection, Map, ClosureEnv
ori_rc_free, GEP, recursive dec, per-type drop
IsShared, is_shared, refcount check, unique, shared
reuse, reset/reuse, fast path, slow path, Construct
PartialApply, closure environment, env_ptr, captures
ori_rc_alloc, environment struct, wrapper function
single-threaded, --single-threaded, non-atomic fast path
```

---

### Section 02: Optimization Enhancements
**File:** `section-02-optimization.md` | **Status:** Not Started

```
RcIdentityMap, RC identity, canonical root, transitive closure
BorrowedFrom, DerivedOwnership, projection, alias
known-safe, guarding pair, GuardInterval, nested retain/release
elimination candidate, inner pair, guard interval
batched inc, count reduction, RcInc count, multi-dec
identity-aware, same_identity, elimination phase
inter-function, borrow signature, Salsa tracked, AnnotatedSig
invalidation, incremental, caller recomputation
```

---

### Section 03: Verification & Enforcement
**File:** `section-03-verification.md` | **Status:** Not Started

```
FBIP, @fbip, enforcement, functional in-place
missed reuse, MissedReuse, FbipReport, compile error
dual execution, JIT vs AOT, RC verification
conservative RC, optimized RC, output divergence
capability-aware, capability-free, borrow inference
no deallocation, side-effect-free, aggressive optimization
RC statistics, --emit-arc-stats, JSON, regression tracking
cross-function, call boundary, callee borrow, elide Inc/Dec
```

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| 01 | LLVM Codegen Completeness | `section-01-codegen.md` |
| 02 | Optimization Enhancements | `section-02-optimization.md` |
| 03 | Verification & Enforcement | `section-03-verification.md` |
