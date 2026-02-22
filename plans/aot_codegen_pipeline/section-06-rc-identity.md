---
section: "06"
title: "RC Identity Propagation"
status: not-started
goal: "Normalize RC operations to canonical root identities, enabling more elimination opportunities"
inspired_by:
  - "Swift RCIdentityFunctionInfo (SILOptimizer/ARC/RCIdentityAnalysis.h)"
  - "Lean 4 DerivedValMap (Compiler/IR/Borrow.lean)"
sections:
  - id: "06.1"
    title: "Build RcIdentityMap"
    status: not-started
  - id: "06.2"
    title: "Propagate identities in RC ops"
    status: not-started
  - id: "06.3"
    title: "Integrate into pipeline"
    status: not-started
  - id: "06.4"
    title: "Tests"
    status: not-started
---

# Section 06: RC Identity Propagation

**Status:** Not Started
**Goal:** `RcInc(x.field)` and `RcInc(x)` are recognized as the same RC identity, enabling more Inc/Dec pair elimination.

**Context:** Ori's `DerivedOwnership::BorrowedFrom(root)` already tracks projection chains (e.g., `x.0` borrows from `x`). But this information isn't used during RC elimination. When the RC inserter adds `RcInc` for a projected field and `RcDec` for its root, the eliminator can't see they're the same identity. Swift's `RCIdentityFunctionInfo` solves this by normalizing all projections to their canonical root before elimination.

**Key insight:** This is a ~150-line pass that unlocks significant elimination without any new dataflow analysis. It reuses information already computed by the borrow inference.

---

## 06.1 Build RcIdentityMap

**File:** `compiler/ori_arc/src/rc_identity.rs` (NEW)

- [ ] Create the module with the identity map:
  ```rust
  use crate::ir::{ArcFunction, ArcVarId};
  use crate::ownership::DerivedOwnership;

  /// Maps each variable to its canonical RC identity root.
  /// Built from DerivedOwnership in a single pass.
  ///
  /// When `x.field` is projected from `x`, both have the same RC identity.
  /// This means `RcInc(x.field)` and `RcDec(x)` cancel each other.
  pub struct RcIdentityMap {
      /// identity[var.index()] = canonical root for RC purposes.
      /// Variables that ARE their own root map to themselves.
      identity: Vec<ArcVarId>,
  }

  impl RcIdentityMap {
      pub fn build(func: &ArcFunction, ownership: &[DerivedOwnership]) -> Self {
          let n = func.var_types.len();
          let mut identity: Vec<ArcVarId> = (0..n)
              .map(|i| ArcVarId::new(i as u32))
              .collect();

          // Resolve projection chains to their roots.
          // Use iterative resolution for chains like a.b.c → a.b → a.
          for (i, own) in ownership.iter().enumerate() {
              if let DerivedOwnership::BorrowedFrom(root) = own {
                  if *root != ArcVarId::new(i as u32) {
                      identity[i] = Self::resolve_root(*root, &identity);
                  }
              }
          }

          Self { identity }
      }

      /// Follow the chain to the ultimate root.
      fn resolve_root(var: ArcVarId, identity: &[ArcVarId]) -> ArcVarId {
          let mut current = var;
          // Follow at most N steps to prevent infinite loops (shouldn't happen,
          // but defensive programming for cyclic ownership bugs).
          for _ in 0..identity.len() {
              let parent = identity[current.index()];
              if parent == current {
                  return current;
              }
              current = parent;
          }
          current
      }

      /// Get the canonical RC identity for a variable.
      pub fn root(&self, var: ArcVarId) -> ArcVarId {
          self.identity[var.index()]
      }

      /// True if this variable IS the root (not a projection).
      pub fn is_root(&self, var: ArcVarId) -> bool {
          self.identity[var.index()] == var
      }
  }
  ```

---

## 06.2 Propagate Identities in RC Operations

**File:** `compiler/ori_arc/src/rc_identity.rs`

- [ ] Add the propagation pass:
  ```rust
  /// Normalize RC operations to use canonical root identities.
  ///
  /// After this pass:
  /// - `RcInc(projected_field)` becomes `RcInc(root_owner)`
  /// - `RcDec(projected_field)` becomes `RcDec(root_owner)`
  ///
  /// This enables the elimination pass to find more matching Inc/Dec pairs
  /// because projections and their roots are now treated as identical.
  ///
  /// Safety: Only valid when the root is provably alive whenever the
  /// projection is alive (guaranteed by DerivedOwnership::BorrowedFrom).
  pub fn propagate_rc_identity(func: &mut ArcFunction, identity_map: &RcIdentityMap) {
      for block in &mut func.blocks {
          for instr in &mut block.body {
              match instr {
                  ArcInstr::RcInc { var, .. } => {
                      let root = identity_map.root(*var);
                      if root != *var {
                          tracing::trace!(
                              from = ?var,
                              to = ?root,
                              "normalizing RcInc to root identity"
                          );
                          *var = root;
                      }
                  }
                  ArcInstr::RcDec { var } => {
                      let root = identity_map.root(*var);
                      if root != *var {
                          tracing::trace!(
                              from = ?var,
                              to = ?root,
                              "normalizing RcDec to root identity"
                          );
                          *var = root;
                      }
                  }
                  _ => {}
              }
          }
      }
  }
  ```

---

## 06.3 Integrate into Pipeline

**File:** `compiler/ori_arc/src/lib.rs`

- [ ] Add the pass to `run_arc_pipeline()` between reuse expansion and RC elimination:
  ```rust
  // ... existing passes ...
  expand_reset_reuse(&mut func, classifier);

  // NEW: Normalize RC identities before elimination
  let identity_map = RcIdentityMap::build(&func, &ownership);
  propagate_rc_identity(&mut func, &identity_map);

  // RC elimination now sees normalized identities
  eliminate_rc_ops_dataflow(&mut func, &ownership);
  ```

- [ ] Add `mod rc_identity;` to `lib.rs`

- [ ] Ensure the pass is idempotent (running it twice produces the same result)

---

## 06.4 Tests

**File:** `compiler/ori_arc/src/rc_identity/tests.rs`

- [ ] Test `RcIdentityMap::build`:
  - Simple projection: `let a = ...; let b = a.0;` → `identity[b] == a`
  - Chain projection: `let a = ...; let b = a.0; let c = b.1;` → `identity[c] == a`
  - Independent vars: `let a = ...; let b = ...;` → `identity[a] == a, identity[b] == b`
  - Mixed: some projected, some independent

- [ ] Test `propagate_rc_identity`:
  - `RcInc(a.0)` becomes `RcInc(a)` when `a.0` borrows from `a`
  - `RcDec(a.0)` becomes `RcDec(a)` when `a.0` borrows from `a`
  - Non-projected vars left unchanged
  - Pass is idempotent

- [ ] Integration test:
  - Function with `let t = (x, y); let a = t.0; RcInc(a); ... RcDec(t);`
  - After identity propagation, `RcInc(a)` becomes `RcInc(t)`
  - After RC elimination, the `RcInc(t)/RcDec(t)` pair is eliminated

- [ ] Run `./test-all.sh` — no regressions

---

## 06.5 Completion Checklist

- [ ] `RcIdentityMap` type defined in `rc_identity.rs`
- [ ] `propagate_rc_identity` pass implemented
- [ ] Integrated into `run_arc_pipeline` between expansion and elimination
- [ ] Chain resolution handles multi-level projections
- [ ] Defensive loop bound prevents infinite resolution
- [ ] Tracing output for normalized identities
- [ ] Unit tests for map building, propagation, and idempotency
- [ ] Integration test showing elimination improvement
- [ ] `./test-all.sh` passes

**Exit Criteria:** `propagate_rc_identity` runs without error on all existing test programs. At least one integration test demonstrates an Inc/Dec pair that was NOT eliminated before but IS eliminated after identity propagation.
