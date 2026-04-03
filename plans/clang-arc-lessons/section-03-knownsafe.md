---
section: "03"
title: "KnownSafe Nested Pair Elimination"
status: not-started
reviewed: false
goal: "Post-emission analysis proves physical refcount positivity within retain/release brackets, eliminating redundant inner RC pairs regardless of intervening side effects"
inspired_by:
  - "LLVM PtrState.h KnownSafe flag (llvm-project/llvm/lib/Transforms/ObjCARC/PtrState.h:55-95)"
  - "LLVM ObjCARCOpts.cpp PairUpRetainsAndReleases (llvm-project/llvm/lib/Transforms/ObjCARC/ObjCARCOpts.cpp:1812-1960)"
  - "Swift RefCountState.h KnownSafe + LatticeState (swift/lib/SILOptimizer/ARC/RefCountState.h:55-186)"
  - "Codex correction: 'KnownSafe comes from positive-refcount sequencing, not from Cardinality demand'"
depends_on: ["01", "02"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "03.1"
    title: "Define Physical Refcount State Model"
    status: not-started
  - id: "03.2"
    title: "Implement Forward Refcount Tracking"
    status: not-started
  - id: "03.3"
    title: "Implement KnownSafe Pair Elimination Pass"
    status: not-started
  - id: "03.4"
    title: "Integration and Matrix Testing"
    status: not-started
  - id: "03.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "03.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 03: KnownSafe Nested Pair Elimination

**Status:** Not Started
**Goal:** Add a post-emission optimization pass that tracks physical refcount positivity per variable and eliminates inner `RcInc`/`RcDec` pairs that are nested within outer pairs guaranteeing the refcount stays positive. This is the LLVM `KnownSafe` optimization adapted to Ori's typed ARC IR.

**Context:** When a variable has refcount known to be > 1 (because an outer `RcInc` is in scope and no intervening operation could decrement it to zero), inner `RcInc`/`RcDec` pairs on the same variable are provably redundant — the refcount can never hit zero regardless of any intervening side effects. LLVM's ARC optimizer uses the `KnownSafe` flag on `PtrState` for this: once a retain is seen and no release has occurred, the refcount is "known positive" and nested pairs can be eliminated.

**Critical design choice (from Codex):** This is NOT about AIMS `Cardinality`. `Cardinality::Once` means "used once more in the future" — that's *demand*, not *proof of positive refcount right now*. KnownSafe is a *physical* fact about the runtime refcount at a specific program point, computed post-emission from the actual `RcInc`/`RcDec` instructions in the IR.

**Reference implementations:**
- **LLVM** `PtrState.h:55-95`: `RRInfo::KnownSafe` — "KnownSafe is true when the refcount is known to be positive, either because we're inside a retain/release pair on the same pointer, or because nested retain/release pairs exist."
- **LLVM** `ObjCARCOpts.cpp:1812-1960`: `PairUpRetainsAndReleases()` uses `KnownSafeTD` (top-down from retain) and `KnownSafeBU` (bottom-up from release) — both must be true for elimination.
- **Swift** `RefCountState.h:55-186`: `KnownSafe` + `LatticeState` enum (`None | Decremented | MightBeUsed | MightBeDecremented`).

**Depends on:** Section 01 (statistics to measure elimination), Section 02 (selective barriers change which RC ops reach this pass).

---

## 03.1 Define Physical Refcount State Model

**File(s):** `compiler/ori_arc/src/aims/knownsafe/mod.rs` (new module)

Define the per-variable physical refcount state tracked during post-emission analysis. This is distinct from the AIMS lattice — it tracks the *physical* refcount state of already-emitted RC operations.

- [ ] Create `compiler/ori_arc/src/aims/knownsafe/` directory with `mod.rs`

- [ ] Define `PhysicalRcState`:
  ```rust
  /// Physical refcount state for a single variable at a program point.
  ///
  /// Tracks whether the refcount is known to be positive (> 0) based on
  /// the emitted RcInc/RcDec instructions. This is a runtime fact, not
  /// a semantic demand (unlike AIMS Cardinality).
  #[derive(Clone, Copy, Debug, PartialEq, Eq)]
  pub enum PhysicalRcState {
      /// Refcount state is unknown (conservative).
      Unknown,
      /// Refcount is known to be positive (> 0) because we're inside
      /// an RcInc bracket that hasn't been decremented.
      Positive,
      /// Refcount may have been decremented (an RcDec was seen without
      /// a subsequent RcInc to restore positivity).
      MaybeDecremented,
  }
  ```

- [ ] Define `VarRcInfo`:
  ```rust
  /// Per-variable refcount tracking info at a program point.
  #[derive(Clone, Debug)]
  pub struct VarRcInfo {
      /// Current physical refcount state.
      pub state: PhysicalRcState,
      /// Whether inner RcInc/RcDec pairs can be safely eliminated.
      /// True when `state == Positive` — the refcount can never hit zero
      /// regardless of inner operations.
      pub known_safe: bool,
      /// Net refcount delta since last "reset" point (function entry or
      /// control flow merge). Used to track nesting depth.
      pub net_delta: i32,
  }
  ```

- [ ] Define monotonic merge for control flow joins:
  ```rust
  impl VarRcInfo {
      /// Conservative merge at control flow join points.
      /// KnownSafe is only true if ALL predecessors agree.
      pub fn merge(&self, other: &VarRcInfo) -> VarRcInfo {
          VarRcInfo {
              state: match (self.state, other.state) {
                  (PhysicalRcState::Positive, PhysicalRcState::Positive) => PhysicalRcState::Positive,
                  _ => PhysicalRcState::Unknown,
              },
              known_safe: self.known_safe && other.known_safe,
              net_delta: self.net_delta.min(other.net_delta),
          }
      }
  }
  ```

- [ ] Unit tests for `PhysicalRcState` and `VarRcInfo::merge()`.

---

## 03.2 Implement Forward Refcount Tracking

**File(s):** `compiler/ori_arc/src/aims/knownsafe/analysis.rs` (new file)

Walk the post-emission ARC IR forward (top-down through instructions), tracking `VarRcInfo` per variable per block. Note: despite Clang's "bottom-up" terminology for its release-scanning pass, this analysis walks instructions *forward* to track refcount positivity state.

- [ ] Define `KnownSafeAnalysis`:
  ```rust
  pub struct KnownSafeAnalysis {
      /// Per-block, per-variable refcount info at block exit.
      block_states: FxHashMap<ArcBlockId, FxHashMap<ArcVarId, VarRcInfo>>,
  }
  ```

- [ ] Implement forward walk per block (uses Section 02's `callee_may_observe_rc()`):
  ```rust
  fn analyze_block(
      &mut self,
      func: &ArcFunction,
      block_idx: usize,
      contracts: &FxHashMap<Name, MemoryContract>,
  ) {
      let block = &func.blocks[block_idx];
      let mut var_states: FxHashMap<ArcVarId, VarRcInfo> = self.entry_state(block_idx);
      
      for instr in &block.body {
          match instr {
              ArcInstr::RcInc { var, count, .. } => {
                  let info = var_states.entry(*var).or_default();
                  info.net_delta += *count as i32;
                  if info.net_delta > 0 {
                      info.state = PhysicalRcState::Positive;
                      info.known_safe = true;
                  }
              }
              ArcInstr::RcDec { var, .. } => {
                  let info = var_states.entry(*var).or_default();
                  info.net_delta -= 1;
                  if info.net_delta <= 0 {
                      info.state = PhysicalRcState::MaybeDecremented;
                      info.known_safe = false;
                  }
              }
              // Calls: use Section 02's callee_may_observe_rc() to determine
              // which variables' KnownSafe survives across the call.
              ArcInstr::Apply { func, args, .. } => {
                  let contract = contracts.get(func);
                  for (var, info) in var_states.iter_mut() {
                      // If the variable is passed as an arg to this call,
                      // check if the callee can observe its RC.
                      let is_arg = args.iter().enumerate().any(|(idx, &a)| {
                          a == *var && callee_may_observe_rc(contract, idx)
                      });
                      if is_arg {
                          info.known_safe = false;
                          info.state = PhysicalRcState::Unknown;
                      }
                      // Variables NOT passed as args, or passed to Borrowed
                      // params, retain their KnownSafe status.
                  }
              }
              ArcInstr::ApplyIndirect { .. } => {
                  // No contract for indirect calls — conservative reset.
                  for info in var_states.values_mut() {
                      info.known_safe = false;
                      info.state = PhysicalRcState::Unknown;
                  }
              }
              _ => {}
          }
      }
      
      self.block_states.insert(ArcBlockId(block_idx), var_states);
  }
  ```

- [ ] Handle control flow merges using `VarRcInfo::merge()`.

- [ ] Unit tests: simple linear block, diamond CFG, loop.

- [ ] **TPR checkpoint** — `/tpr-review` covering 03.1–03.2 implementation work

---

## 03.3 Implement KnownSafe Pair Elimination Pass

**File(s):** `compiler/ori_arc/src/aims/knownsafe/eliminate.rs` (new file)

Use the analysis results to eliminate redundant inner RC pairs.

- [ ] Define `eliminate_knownsafe_pairs()`:
  ```rust
  /// Remove RcInc/RcDec pairs where KnownSafe is true for the variable.
  ///
  /// A pair is eliminable when:
  /// 1. An RcInc(x) is followed by RcDec(x) in the same block
  /// 2. x.known_safe is true at the RcInc point (refcount known > 0)
  /// 3. No instruction between them can make x's refcount hit zero
  ///
  /// Condition 3 is guaranteed by KnownSafe — if the refcount is known
  /// positive, it cannot hit zero regardless of intervening operations.
  pub fn eliminate_knownsafe_pairs(
      func: &mut ArcFunction,
      analysis: &KnownSafeAnalysis,
  ) -> usize  // returns number of pairs eliminated
  ```

- [ ] Elimination algorithm:
  1. Walk each block forward
  2. At each `RcInc(x)`, check if `x.known_safe` at this point
  3. If yes, scan forward for matching `RcDec(x)` in same block
  4. If found with no intervening use that requires positive refcount, eliminate both
  5. Track eliminated count for statistics

- [ ] Unit tests:
  - Nested `RcInc(x); RcInc(x); use(x); RcDec(x); RcDec(x)` → inner pair eliminated
  - `RcInc(x); call(); RcDec(x)` → NOT eliminated (call may observe)
  - `RcInc(x); RcInc(x); call(); RcDec(x); RcDec(x)` with call barrier reset → inner pair NOT eliminated
  - Two separate variables, only one known_safe → correct variable's pair eliminated

**Semantic pin:** Test that creates nested inc/dec pattern, verifies inner pair is eliminated, and the resulting program still runs correctly with `ORI_CHECK_LEAKS=1`.

---

## 03.4 Integration and Matrix Testing

**File(s):** `compiler/ori_arc/src/pipeline/aims_pipeline.rs`, tests

Integrate the KnownSafe pass into the AIMS pipeline and verify correctness.

- [ ] **Prerequisite: `aims_pipeline.rs` splitting** (see overview Prerequisites table). The pipeline file is 590 lines (exceeds 500-line limit). Complete the prerequisite extraction before adding the KnownSafe pass invocation.

- [ ] Insert KnownSafe analysis + elimination into `run_aims_pipeline()`:
  - After step 5 (`realize_rc_reuse`) and before step 6 (`verify`)
  - This is post-emission, pre-verification — the right place for a post-emission optimization

- [ ] Add `knownsafe_pairs_eliminated` to `SynergyMetrics`

- [ ] **Matrix testing:**
  - **Type dimension**: str, [int], Option\<str\>, closures, structs with heap fields, maps
  - **Pattern dimension**: nested function calls, loop with inner RC, match arms with shared RC, chained method calls
  - **Nesting dimension**: 1-deep (no elimination), 2-deep (one pair), 3-deep (two pairs)

- [ ] **Correctness verification:**
  - `ORI_CHECK_LEAKS=1` on all test programs → zero leaks
  - Dual-exec parity: interpreter and LLVM produce identical results
  - Debug and release builds produce identical results

- [ ] **Measurement:**
  - `ORI_LOG=ori_arc=info ori build tests/spec/` → report `knownsafe_pairs_eliminated`
  - Target: 5-15% reduction in total RC operations on programs with nested calls

- [ ] **Verify tests pass in debug and release**

---

## 03.R Third Party Review Findings

- None.

---

## 03.N Completion Checklist

- [ ] `PhysicalRcState` enum with `Unknown`, `Positive`, `MaybeDecremented`
- [ ] `VarRcInfo` with monotonic merge at control flow joins
- [ ] `KnownSafeAnalysis` forward walk produces correct per-block state
- [ ] `eliminate_knownsafe_pairs()` removes inner pairs within known-positive brackets
- [ ] Nested inc/dec patterns correctly identified and eliminated
- [ ] Non-nested patterns correctly preserved (no over-elimination)
- [ ] `knownsafe_pairs_eliminated` tracked in `SynergyMetrics`
- [ ] `ORI_CHECK_LEAKS=1` reports zero leaks on all test programs
- [ ] Dual-exec parity maintained (interpreter == LLVM)
- [ ] `./test-all.sh` green (debug + release)
- [ ] No spurious warnings in normal compilation
- [ ] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan 03` returns 0 annotations
- [ ] All intermediate TPR checkpoint findings resolved
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues
- [ ] `/impl-hygiene-review last commit` passed — hygiene review clean

**Exit Criteria:** `ORI_LOG=ori_arc=info ori build` shows `knownsafe_pairs_eliminated > 0` for programs with nested function calls on heap-typed values. All existing tests pass. `ORI_CHECK_LEAKS=1` reports zero leaks. Dual-exec parity maintained.
