---
section: "07"
title: "Cross-Block RC Elimination"
status: not-started
goal: "Extend RC elimination from intra-block to cross-block and known-safe guarding pairs"
inspired_by:
  - "Swift ARCSequenceOpts (bidirectional dataflow + matching)"
  - "Swift 'Known Safe' guarding pair detection"
depends_on: ["06"]
sections:
  - id: "07.1"
    title: "Single-predecessor cross-block elimination"
    status: not-started
  - id: "07.2"
    title: "Known-safe guarding pair detection"
    status: not-started
  - id: "07.3"
    title: "Join point convergence"
    status: not-started
  - id: "07.4"
    title: "Tests & verification"
    status: not-started
---

# Section 07: Cross-Block RC Elimination

**Status:** Not Started
**Goal:** Eliminate matching `RcInc`/`RcDec` pairs across basic block boundaries and detect provably-safe inner pairs bracketed by guarding operations.

**Context:** The current `eliminate_rc_ops_dataflow` in `rc_elim/mod.rs` (631 lines) is intra-block only: it finds matching Inc/Dec pairs within a single basic block. The DPR found that Swift's ARC optimizer uses a three-phase approach (bottom-up discovery, top-down discovery, matching) to eliminate across blocks. Ori doesn't need the full 5.7K-line Swift optimizer, but can gain significant benefit from two targeted extensions.

**Depends on:** Section 06 (RC Identity Propagation) — normalized identities make cross-block matching more effective.

---

## 07.1 Single-Predecessor Cross-Block Elimination

**File:** `compiler/ori_arc/src/rc_elim/mod.rs`

This is the simplest cross-block extension: when block B has exactly one predecessor A, an `RcInc` at the end of A and a matching `RcDec` at the start of B can be eliminated as if they were in the same block.

- [ ] Add `cross_block_single_pred_elim()`:
  ```rust
  /// Eliminate RcInc/RcDec pairs across single-predecessor block boundaries.
  ///
  /// When block B has exactly one predecessor A:
  ///   A: ... RcInc(x) → Jump(B)
  ///   B: RcDec(x) ...
  /// The pair can be eliminated because the control flow is linear.
  fn cross_block_single_pred_elim(func: &mut ArcFunction) -> usize {
      let predecessors = compute_predecessor_map(func);
      let mut eliminated = 0;

      for (block_id, preds) in &predecessors {
          if preds.len() != 1 {
              continue;
          }
          let pred_id = preds[0];

          // Find trailing RcInc ops in predecessor
          let pred_block = &func.blocks[pred_id.index()];
          let trailing_incs = collect_trailing_rc_incs(pred_block);

          // Find leading RcDec ops in this block
          let this_block = &func.blocks[block_id.index()];
          let leading_decs = collect_leading_rc_decs(this_block);

          // Match pairs by variable identity
          for (var, inc_idx) in &trailing_incs {
              if let Some(dec_idx) = leading_decs.get(var) {
                  // Verify no intervening use of `var` between the Inc and Dec
                  if no_intervening_use(*var, pred_block, *inc_idx, this_block, *dec_idx) {
                      // Mark both for removal
                      mark_for_removal(func, pred_id, *inc_idx);
                      mark_for_removal(func, *block_id, *dec_idx);
                      eliminated += 1;
                  }
              }
          }
      }

      // Sweep: remove marked instructions
      sweep_marked(func);
      eliminated
  }
  ```

- [ ] Add `compute_predecessor_map()` utility (or reuse from dominator tree):
  - Walk all terminators, record edges
  - Return `FxHashMap<ArcBlockId, Vec<ArcBlockId>>`

- [ ] Integrate into `eliminate_rc_ops_dataflow`:
  - Run intra-block elimination first (existing)
  - Then run single-predecessor cross-block elimination
  - Iterate until no more eliminations (fixed point)

---

## 07.2 Known-Safe Guarding Pair Detection

**File:** `compiler/ori_arc/src/rc_elim/mod.rs`

Swift's "Known Safe" optimization: when an outer `RcInc`/`RcDec` pair on variable `x` brackets a region, any inner `RcInc`/`RcDec` pair on the same `x` is provably redundant — the outer pair guarantees `x` stays alive throughout.

- [ ] Add `known_safe_guarding_elim()`:
  ```rust
  /// Eliminate inner RcInc/RcDec pairs that are guarded by outer pairs.
  ///
  /// Pattern:
  ///   RcInc(x)        ← outer inc (guard)
  ///   ...
  ///   RcInc(x)        ← inner inc (redundant)
  ///   ... use of x ...
  ///   RcDec(x)        ← inner dec (redundant)
  ///   ...
  ///   RcDec(x)        ← outer dec (guard)
  ///
  /// The inner pair is safe to eliminate because the outer pair ensures
  /// x's refcount never reaches 0 in the bracketed region.
  fn known_safe_guarding_elim(func: &mut ArcFunction) -> usize {
      let mut eliminated = 0;

      for block in &mut func.blocks {
          // Track "active guards": variables with unmatched RcInc
          let mut guards: FxHashMap<ArcVarId, usize> = FxHashMap::default();
          // Track inner pairs that are guarded
          let mut guarded_pairs: Vec<(usize, usize)> = Vec::new();

          for (idx, instr) in block.body.iter().enumerate() {
              match instr {
                  ArcInstr::RcInc { var, .. } => {
                      if guards.contains_key(var) {
                          // This is an inner inc — look for matching inner dec
                          // (will be paired during the Dec scan)
                      }
                      *guards.entry(*var).or_insert(0) += 1;
                  }
                  ArcInstr::RcDec { var } => {
                      if let Some(count) = guards.get_mut(var) {
                          if *count > 1 {
                              // This dec matches an inner inc (guarded)
                              // Find the matching inner inc and mark both
                              // ...
                              *count -= 1;
                              eliminated += 1;
                          } else {
                              // This dec matches the outer guard inc
                              guards.remove(var);
                          }
                      }
                  }
                  _ => {}
              }
          }
      }

      eliminated
  }
  ```

- [ ] This is initially intra-block only (matching the existing elimination scope), but the pattern naturally extends to cross-block with the predecessor map.

---

## 07.3 Join Point Convergence

**File:** `compiler/ori_arc/src/rc_elim/mod.rs`

When multiple predecessors converge at a join point (block with >1 predecessor), RC state must be reconciled. This is the hardest part — Lean uses explicit block parameters, Swift uses lattice merging.

- [ ] Define RC state lattice for blocks:
  ```rust
  /// RC state for a variable at a block boundary.
  #[derive(Clone, Copy, PartialEq, Eq)]
  enum RcState {
      /// No outstanding RC operations.
      Neutral,
      /// One unmatched RcInc (value has been incremented).
      Incremented,
      /// One unmatched RcDec (value may be decremented).
      Decremented,
      /// Conflicting — cannot eliminate across this join.
      Unknown,
  }
  ```

- [ ] Implement lattice merge:
  ```rust
  fn merge(a: RcState, b: RcState) -> RcState {
      if a == b { a } else { RcState::Unknown }
  }
  ```

- [ ] Build per-block entry/exit RC state maps:
  - Walk blocks in reverse postorder
  - For each block, compute exit state from entry state + instructions
  - At join points, merge predecessor exit states into entry state
  - Iterate until fixed point

- [ ] Use converged states to identify safe cross-block eliminations at joins

**Note:** This subsection is the most complex. If it proves too expensive, defer to Phase 3+ and keep only 07.1 and 07.2 which provide significant benefit at lower complexity.

---

## 07.4 Tests & Verification

- [ ] Unit tests for single-predecessor elimination:
  - Linear chain: `A → B` with matching Inc/Dec across boundary
  - Chain with intervening use (should NOT eliminate)
  - Multiple pairs across same boundary

- [ ] Unit tests for known-safe guarding:
  - Simple: outer Inc/Dec brackets inner Inc/Dec on same var
  - Nested: three levels of guarding
  - Different vars: outer guards x, inner operates on y (no elimination)

- [ ] Unit tests for join convergence (if implemented):
  - Diamond: `A → B, A → C, B → D, C → D` with matching Inc/Dec
  - Conflicting: one predecessor increments, other doesn't

- [ ] Integration test showing real-world improvement:
  - Function that creates a struct, passes it through if/else, returns it
  - Before: 4 RC ops. After cross-block: 2 RC ops.

- [ ] Verify `./test-all.sh` — zero regressions

---

## 07.5 Completion Checklist

- [ ] `cross_block_single_pred_elim` implemented and integrated
- [ ] `known_safe_guarding_elim` implemented and integrated
- [ ] Join point convergence implemented (or explicitly deferred with rationale)
- [ ] Predecessor map utility available
- [ ] Fixed-point iteration between intra-block and cross-block passes
- [ ] All tests pass, including new elimination-specific tests
- [ ] Tracing output showing eliminated pairs with source locations
- [ ] `./test-all.sh` green

**Exit Criteria:** At least 20% more RC operations eliminated in the AOT test suite compared to intra-block-only elimination. Measured by adding an `eliminated_count` metric to `run_arc_pipeline`.
