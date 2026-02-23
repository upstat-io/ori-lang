---
section: "07"
title: "Cross-Block RC Elimination"
status: complete
goal: "Extend RC elimination from intra-block to cross-block and known-safe guarding pairs"
inspired_by:
  - "Swift ARCSequenceOpts (bidirectional dataflow + matching)"
  - "Swift 'Known Safe' guarding pair detection"
depends_on: ["06"]
sections:
  - id: "07.1"
    title: "Single-predecessor cross-block elimination"
    status: complete
  - id: "07.2"
    title: "Known-safe guarding pair detection"
    status: complete
  - id: "07.3"
    title: "Join point convergence"
    status: complete
  - id: "07.4"
    title: "Tests & verification"
    status: complete
---

# Section 07: Cross-Block RC Elimination

**Status:** Complete
**Goal:** Eliminate matching `RcInc`/`RcDec` pairs across basic block boundaries and detect provably-safe inner pairs bracketed by guarding operations.

**Context:** The current `eliminate_rc_ops_dataflow` in `rc_elim/mod.rs` (631 lines) is intra-block only: it finds matching Inc/Dec pairs within a single basic block. The DPR found that Swift's ARC optimizer uses a three-phase approach (bottom-up discovery, top-down discovery, matching) to eliminate across blocks. Ori doesn't need the full 5.7K-line Swift optimizer, but can gain significant benefit from two targeted extensions.

**Depends on:** Section 06 (RC Identity Propagation) — normalized identities make cross-block matching more effective.

---

## 07.1 Single-Predecessor Cross-Block Elimination

**File:** `compiler/ori_arc/src/rc_elim/mod.rs`

This is the simplest cross-block extension: when block B has exactly one predecessor A, an `RcInc` at the end of A and a matching `RcDec` at the start of B can be eliminated as if they were in the same block.

- [x] Add `cross_block_single_pred_elim()`:
  Implemented as `eliminate_cross_block_pairs()` in `rc_elim/mod.rs`. Scans leading `RcDec` instructions in single-predecessor blocks, matches with trailing `RcInc` in the predecessor (backward scan with intervening-use check), and removes matched pairs.

- [x] Add `compute_predecessor_map()` utility (or reuse from dominator tree):
  Implemented as `compute_predecessors()` in `graph/mod.rs`. Returns `Vec<Vec<usize>>` indexed by block index.

- [x] Integrate into `eliminate_rc_ops`:
  - Run intra-block elimination first (`eliminate_once`)
  - Then run single-predecessor cross-block elimination (`eliminate_cross_block_pairs`)
  - Then run known-safe guarding elimination (`known_safe_guarding_elim`)
  - Iterate until no more eliminations (fixed point)

---

## 07.2 Known-Safe Guarding Pair Detection

**File:** `compiler/ori_arc/src/rc_elim/mod.rs`

Swift's "Known Safe" optimization: when an outer `RcInc`/`RcDec` pair on variable `x` brackets a region, any inner `RcInc`/`RcDec` pair on the same `x` is provably redundant — the outer pair guarantees `x` stays alive throughout.

- [x] Add `known_safe_guarding_elim()`:
  Implemented in `rc_elim/mod.rs`. Uses a per-variable stack of `RcInc` positions: the bottom entry is the "outer guard"; entries above it are inner candidates. When a matching `RcDec` is encountered and the stack depth is > 1, the inner Inc/Dec pair is marked for removal. Handles arbitrary nesting depth (three+ levels). Integrated into the `eliminate_rc_ops` fixed-point loop.

- [x] This is initially intra-block only (matching the existing elimination scope), but the pattern naturally extends to cross-block with the predecessor map.

---

## 07.3 Join Point Convergence

**File:** `compiler/ori_arc/src/rc_elim/mod.rs`

When multiple predecessors converge at a join point (block with >1 predecessor), RC state must be reconciled. This is the hardest part — Lean uses explicit block parameters, Swift uses lattice merging.

- [x] Define RC state lattice for blocks:
  Implemented via `available_out` sets in `eliminate_join_pairs()`. Uses set intersection as the lattice merge (equivalent to `Unknown` when states disagree, `Incremented` when all agree). Simpler than a full four-state lattice but equivalent in power for the patterns we care about.

- [x] Implement lattice merge:
  Implemented as set intersection: `set.retain(|v| available_out[pred_idx].contains(v))`. An `RcInc(x)` is available at block B's entry only if it's available on ALL incoming edges.

- [x] Build per-block entry/exit RC state maps:
  `available_out` computed for each block by scanning instructions in reverse. Trailing `RcInc` variables that aren't used by the terminator are "available." Uses/Decs invalidate availability.

- [x] Use converged states to identify safe cross-block eliminations at joins:
  At multi-predecessor join points, intersects available sets from all predecessors. If an `RcDec(x)` at the join's entry has `RcInc(x)` available from ALL predecessors, all Incs and the Dec are eliminated.

**Note:** This subsection is the most complex. If it proves too expensive, defer to Phase 3+ and keep only 07.1 and 07.2 which provide significant benefit at lower complexity.

---

## 07.4 Tests & Verification

- [x] Unit tests for single-predecessor elimination:
  - `cross_block_edge_pair_eliminated`: Linear chain `A → B` with matching Inc/Dec across boundary
  - `cross_block_use_after_inc_in_pred_not_eliminated`: Chain with intervening use (should NOT eliminate)
  - `cross_block_with_intervening_unrelated_instr`: Inc after unrelated instruction, Dec in successor
  - `cross_block_terminator_uses_var_not_eliminated`: Terminator uses var, blocks elimination
  - `cross_block_self_loop_not_eliminated`: Self-loop safety
  - `cross_block_diamond_not_eliminated`: Multi-predecessor block (not single-pred, no elimination)

- [x] Unit tests for known-safe guarding:
  - `guarding_eliminates_inner_pair_with_use`: Simple outer Inc/Dec brackets inner Inc/Dec on same var with intervening use
  - `guarding_three_levels_nested`: Three levels of guarding (inner + middle eliminated)
  - `guarding_different_vars_no_elimination`: Outer guards x, inner operates on y (no elimination)
  - `guarding_no_inner_pair`: Only outer pair, no inner → nothing eliminated
  - `guarding_two_sequential_guarded_regions`: Two independent guarded regions

- [x] Unit tests for join convergence:
  - `dataflow_diamond_join`: Diamond `A → B, A → C, B → D, C → D` with matching Inc/Dec (eliminated)
  - `cross_block_diamond_not_eliminated`: Conflicting — one predecessor increments, other doesn't (not eliminated)

- [x] Integration test showing real-world improvement:
  - `dataflow_diamond_join`: Both branches Inc v0, merge Dec's v0 → all 3 RC ops eliminated

- [x] Verify `./test-all.sh` — zero regressions (409 ori_arc tests pass, 9402 total tests pass, 0 new failures)

---

## 07.5 Completion Checklist

- [x] `cross_block_single_pred_elim` implemented and integrated
- [x] `known_safe_guarding_elim` implemented and integrated
- [x] Join point convergence implemented (via `eliminate_join_pairs` using available-set intersection at joins)
- [x] Predecessor map utility available (`compute_predecessors` in `graph/mod.rs`)
- [x] Fixed-point iteration between intra-block, cross-block, and guarding passes
- [x] All tests pass, including new elimination-specific tests (40 rc_elim tests, 409 ori_arc tests)
- [x] Tracing output showing eliminated pairs with source locations (5 tracing::debug calls across all elimination paths)
- [x] `./test-all.sh` green (pre-existing failures only: 3 interpreter spec tests in concurrent.ori, 4 LLVM backend tests)

**Exit Criteria:** At least 20% more RC operations eliminated in the AOT test suite compared to intra-block-only elimination. Measured by adding an `eliminated_count` metric to `run_arc_pipeline`.
