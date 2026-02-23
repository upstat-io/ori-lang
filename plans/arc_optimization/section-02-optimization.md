---
section: "02"
title: Optimization Enhancements
status: not-started
goal: Strengthen RC elimination with identity propagation and known-safe pair detection
sections:
  - id: "2.1"
    title: RC Identity Map Construction
    status: not-started
  - id: "2.2"
    title: Known-Safe Pair Elimination
    status: not-started
  - id: "2.3"
    title: Batched Inc Reduction
    status: not-started
  - id: "2.4"
    title: Identity-Aware Elimination
    status: not-started
  - id: "2.5"
    title: Inter-Function Borrow Caching
    status: not-started
  - id: "2.6"
    title: Completion Checklist
    status: not-started
---

# Section 02: Optimization Enhancements

**Status:** Not Started
**Goal:** Adopt Swift's "Known Safe" RC identity normalization and enhance the elimination pipeline to catch more redundant RC operations.

**Design Reference:** `plans/dpr_arc-optimization_02212026.md` — Phase 2: Core

**Depends on:** Section 01 (codegen must be correct before optimizing it)

---

## 2.1 RC Identity Map Construction

Build a transitive closure over `DerivedOwnership::BorrowedFrom` chains so the elimination pass can recognize that projections (`x.field`) share RC identity with their roots.

- [ ] Create `ori_arc/src/rc_identity.rs` (~80 lines)
- [ ] Implement `RcIdentityMap::build(ownership: &[DerivedOwnership]) -> Self`
  - Chase `BorrowedFrom` chains to transitive roots
  - `Fresh` and `Owned` variables are their own root
  - Include cycle guard for safety
- [ ] Implement `RcIdentityMap::root(var) -> ArcVarId`
- [ ] Implement `RcIdentityMap::same_identity(a, b) -> bool`
- [ ] Add unit tests:
  - Direct borrow: `a borrows b` -> `root(a) == b`
  - Transitive: `a borrows b borrows c` -> `root(a) == root(b) == c`
  - Fresh/Owned are self-rooted
  - Cycle guard doesn't panic

---

## 2.2 Known-Safe Pair Elimination

Implement Swift's "Known Safe" optimization: when an outer `RcInc(guard)` / `RcDec(guard)` pair guarantees an object stays alive, inner Inc/Dec pairs on derived variables can be eliminated without checking for intervening uses.

- [ ] Create `ori_arc/src/rc_elim/known_safe.rs` (~120 lines)
- [ ] Implement `find_guard_intervals(block) -> Vec<GuardInterval>`
  - Identify outermost Inc/Dec pairs per block
- [ ] Implement `find_known_safe_candidates(body, guards, identity_map) -> Vec<EliminationCandidate>`
  - For each guard interval, find inner Inc/Dec pairs on variables with same RC identity
  - These are safe to eliminate regardless of intervening uses
- [ ] Wire as pre-pass in `eliminate_rc_ops_dataflow` (before general elimination)
- [ ] Add tests:
  - Nested struct access: `RcInc(x); RcInc(x.field); use(x.field); RcDec(x.field); RcDec(x)` -> inner pair eliminated
  - Linked-list traversal: field extraction within guarded scope
  - Closure capture within a guarded scope
  - Guard with non-derived variable (not eliminated — different identity)

---

## 2.3 Batched Inc Reduction

Update elimination to reduce `RcInc { count }` by 1 instead of removing the entire instruction when `count > 1`.

- [ ] Implement `reduce_inc_count(instr: &mut ArcInstr) -> bool` (returns true when count reaches 0)
- [ ] Update `apply_eliminations` in `rc_elim` to use `reduce_inc_count` for batched Incs
- [ ] Add tests:
  - `RcInc { var: x, count: 3 }; RcDec { var: x }` -> `RcInc { var: x, count: 2 }`
  - `RcInc { var: x, count: 1 }; RcDec { var: x }` -> both removed
  - Multiple Decs reducing count stepwise

---

## 2.4 Identity-Aware Elimination

Thread `RcIdentityMap` through the general elimination pass so transitive borrow chains are recognized.

- [ ] Thread `&RcIdentityMap` into `eliminate_rc_ops_dataflow`
- [ ] In Phase 2 (ownership elimination), use `identity_map.same_identity(var, source)` instead of direct `BorrowedFrom` check
  - This handles `a borrows b borrows c` — current code only handles single-hop
- [ ] Add tests:
  - Two-hop borrow chain elimination
  - Three-hop chain (edge case)
  - Mixed chains (some hops are `Owned` — should break the chain)

---

## 2.5 Inter-Function Borrow Caching

Wrap borrow signatures in Salsa tracked struct so callee signature changes automatically invalidate callers' RC insertion.

- [ ] Wrap `FxHashMap<Name, AnnotatedSig>` in a Salsa tracked struct
- [ ] Make `infer_borrows` output a Salsa query
- [ ] Make `insert_rc_ops_with_ownership` a dependent Salsa query
- [ ] Add tests:
  - Change callee parameter from `Owned` to `Borrowed` -> caller's RC ops recomputed
  - Unchanged callee -> caller's RC ops cached (no recomputation)

---

## 2.6 Completion Checklist

- [ ] `RcIdentityMap` integrated into elimination pipeline
- [ ] Known-safe elimination removes provably redundant inner pairs
- [ ] Batched Inc properly reduces counts
- [ ] Inter-function borrow caching via Salsa prevents stale RC ops
- [ ] All existing tests pass (`./test-all.sh`)
- [ ] New tests cover each subsection

**Exit Criteria:** The elimination pass catches multi-hop borrow chains and known-safe nested pairs that the current single-hop, no-guard implementation misses.
