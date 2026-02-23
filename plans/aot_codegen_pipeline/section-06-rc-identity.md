---
section: "06"
title: "RC Identity Propagation"
status: complete
goal: "Normalize RC operations to canonical root identities, enabling more elimination opportunities"
inspired_by:
  - "Swift RCIdentityFunctionInfo (SILOptimizer/ARC/RCIdentityAnalysis.h)"
  - "Lean 4 DerivedValMap (Compiler/IR/Borrow.lean)"
sections:
  - id: "06.1"
    title: "Build RcIdentityMap"
    status: complete
  - id: "06.2"
    title: "Propagate identities in RC ops"
    status: complete
  - id: "06.3"
    title: "Integrate into pipeline"
    status: complete
  - id: "06.4"
    title: "Tests"
    status: complete
---

# Section 06: RC Identity Propagation

**Status:** Complete
**Goal:** `RcInc(x.field)` and `RcInc(x)` are recognized as the same RC identity, enabling more Inc/Dec pair elimination.

**Context:** Ori's `DerivedOwnership::BorrowedFrom(root)` already tracks projection chains (e.g., `x.0` borrows from `x`). But this information isn't used during RC elimination. When the RC inserter adds `RcInc` for a projected field and `RcDec` for its root, the eliminator can't see they're the same identity. Swift's `RCIdentityFunctionInfo` solves this by normalizing all projections to their canonical root before elimination.

**Key insight:** This is a ~230-line pass that unlocks significant elimination without any new dataflow analysis. It reuses information already computed by the borrow inference.

---

## 06.1 Build RcIdentityMap

**File:** `compiler/ori_arc/src/rc_identity/mod.rs` (NEW — module directory)

- [x] Create the module with the identity map (`RcIdentityMap` struct)
- [x] `build()` — single-pass construction from `DerivedOwnership` vector
- [x] `resolve_root()` — iterative chain resolution with defensive loop bound
- [x] `root()` — canonical identity lookup with fallback for out-of-bounds vars
- [x] `is_root()` — predicate for root variables
- [x] `normalized_count()` — diagnostic: count of vars that differ from their root

**Implementation note:** The plan's pseudo-code showed `identity[var.index()]` without bounds checking. The actual implementation uses `.get().copied().unwrap_or(var)` in `root()` for safety against expansion passes that introduce vars beyond the initial count.

---

## 06.2 Propagate Identities in RC Operations

**File:** `compiler/ori_arc/src/rc_identity/mod.rs`

- [x] Add the propagation pass (`propagate_rc_identity`)
- [x] Strategy fixup: recompute `RcStrategy` for root variable via `RcStrategy::from_var()`
- [x] Skip normalization when root has `Scalar` repr (no RC needed)
- [x] Safety guard: return `None` from `root_strategy()` when `var_reprs` is empty
- [x] Tracing output for each normalized operation

**Implementation deviation from plan:** The plan's pseudo-code only updates `var` in RcInc/RcDec. The actual implementation also updates `strategy` because projections and their roots can have different `ValueRepr` (e.g., a `str` field extracted from a struct has `FatPointer` strategy while the struct has `AggregateFields`). This required:
1. Adding `pool: &Pool` parameter to `propagate_rc_identity`
2. A `root_strategy()` helper that takes split borrows on `var_reprs` and `var_types`
3. Pre-computing all root strategies into `Vec<Option<RcStrategy>>` before the mutable block iteration to satisfy the borrow checker

---

## 06.3 Integrate into Pipeline

**File:** `compiler/ori_arc/src/lib.rs`

- [x] Add the pass to `run_arc_pipeline()` between reuse expansion and RC elimination:
  ```rust
  expand_reuse::expand_reset_reuse(func, classifier, Some(pool));

  // Normalize RC identities before elimination
  let identity_map = rc_identity::RcIdentityMap::build(func, &ownership);
  rc_identity::propagate_rc_identity(func, &identity_map, pool);

  rc_elim::eliminate_rc_ops_dataflow(func, &ownership);
  ```

- [x] Add `pub mod rc_identity;` to `lib.rs`

- [x] Add `pub use rc_identity::{propagate_rc_identity, RcIdentityMap};` to `lib.rs`

- [x] Ensure the pass is idempotent (running it twice produces the same result) — verified by `propagation_idempotent` test

---

## 06.4 Tests

**File:** `compiler/ori_arc/src/rc_identity/tests.rs`

- [x] Test `RcIdentityMap::build`:
  - `identity_simple_projection`: v1 = Project(v0) → root(v1) == v0
  - `identity_chain_projection`: v0 → v1 → v2, all resolve to v0
  - `identity_independent_vars`: each is own root, normalized_count == 0
  - `identity_fresh_is_root`: Construct-produced vars are their own root
  - `identity_mixed`: combination of projected and independent

- [x] Test `propagate_rc_identity`:
  - `propagation_noop_without_var_reprs`: safety guard blocks normalization when var_reprs empty
  - `propagation_idempotent`: running twice produces same result
  - `propagation_leaves_owned_vars_unchanged`: owned vars never normalized

- [x] Integration test:
  - `integration_identity_enables_pair_elimination`: demonstrates the core value proposition
  - Without propagation: Phase 2 removes RcInc(v1) individually, but RcDec(v0) stays → 1 op remains
  - With propagation: RcInc(v1) → RcInc(v0), Phase 1 pairs Inc(v0)/Dec(v0) → 0 ops remain
  - Asserts `remaining_with < remaining_without` (0 < 1)

- [x] Run `./test-all.sh` — no regressions (9396 passed, 7 pre-existing failures, 120 skipped)
- [x] All 404 ori_arc tests pass, all 845 LLVM tests pass (428 AOT + 360 codegen + 57 unit)

---

## 06.5 Completion Checklist

- [x] `RcIdentityMap` type defined in `rc_identity/mod.rs`
- [x] `propagate_rc_identity` pass implemented
- [x] Integrated into `run_arc_pipeline` between expansion and elimination
- [x] Chain resolution handles multi-level projections
- [x] Defensive loop bound prevents infinite resolution
- [x] Tracing output for normalized identities
- [x] Unit tests for map building, propagation, and idempotency
- [x] Integration test showing elimination improvement
- [x] `./test-all.sh` passes

**Exit Criteria:** `propagate_rc_identity` runs without error on all existing test programs. At least one integration test demonstrates an Inc/Dec pair that was NOT eliminated before but IS eliminated after identity propagation. ✓
