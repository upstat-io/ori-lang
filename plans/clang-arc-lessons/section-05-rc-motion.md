---
section: "05"
title: "PRE-Style Global RC Code Motion"
status: not-started
reviewed: false
goal: "Bidirectional dataflow analysis moves RcInc/RcDec across basic blocks to optimal placement points, with path-counting CFG hazard detection preventing unsafe motion through loops"
inspired_by:
  - "LLVM ObjCARCOpts.cpp VisitBottomUp/VisitTopDown/PairUpRetainsAndReleases (llvm-project/llvm/lib/Transforms/ObjCARC/ObjCARCOpts.cpp:1394-1960)"
  - "LLVM BBState TopDownPathCount/BottomUpPathCount (llvm-project/llvm/lib/Transforms/ObjCARC/ObjCARCOpts.cpp:177-325)"
  - "Swift GlobalARCSequenceDataflow.h region-aware + loop summarization (swift/lib/SILOptimizer/ARC/GlobalARCSequenceDataflow.h)"
  - "LLVM PtrState.h CFGHazardAfflicted separation (llvm-project/llvm/lib/Transforms/ObjCARC/PtrState.h:87)"
depends_on: ["01", "02", "03"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "05.1"
    title: "Define Per-Block RC State Model"
    status: not-started
  - id: "05.2"
    title: "Implement Bottom-Up Pass"
    status: not-started
    note: "Implement AFTER 05.5 path-count computation, or integrate path counts into the BU traversal directly"
  - id: "05.3"
    title: "Implement Top-Down Pass"
    status: not-started
  - id: "05.4"
    title: "Pair Up and Place"
    status: not-started
    note: "Requires 05.5 cfg_hazard flags — must run after path counts are computed"
  - id: "05.5"
    title: "CFG Hazard Detection"
    status: not-started
    note: "Implement before or concurrently with 05.2/05.3 — path counts needed by pairing"
  - id: "05.6"
    title: "Integration and Matrix Testing"
    status: not-started
  - id: "05.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "05.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 05: PRE-Style Global RC Code Motion

**Status:** Not Started
**Goal:** Add a bidirectional dataflow pass that moves `RcInc`/`RcDec` operations across basic block boundaries to find optimal placement points, eliminating partially redundant RC operations. Uses LLVM's bottom-up/top-down pattern with path-counting for CFG hazard detection and Swift's region-awareness for loops.

**Context:** Sections 02-03 optimize RC operations *within* basic blocks (selective barriers, nested pair elimination). This section extends to *cross-block* optimization. LLVM's ARC optimizer performs a bidirectional dataflow: bottom-up from releases to find matching retains, top-down from retains to find matching releases. Both directions must agree for a pair to be eliminated. Path counting ensures operations aren't moved through loop back-edges (which would change execution count). This is the most complex section — it implements a new analysis pass with bidirectional dataflow, CFG hazard checking, and safe code motion.

**Key design principle (from Codex):** "pair safe" (KnownSafe — can this pair be eliminated?) is separate from "motion safe" (CFGHazardAfflicted — can we *move* the ops to do it?). LLVM encodes this as two independent flags on `PtrState`. A pair is eliminated only when BOTH are satisfied.

**Reference implementations:**
- **LLVM** `ObjCARCOpts.cpp:1394-1637`: `VisitBottomUp()` and `VisitTopDown()` — two-pass bidirectional dataflow with per-block state merging.
- **LLVM** `ObjCARCOpts.cpp:177-325`: `BBState` with `TopDownPathCount`/`BottomUpPathCount` — path counting for CFG hazard detection. Overflow → conservative.
- **LLVM** `ObjCARCOpts.cpp:1812-1960`: `PairUpRetainsAndReleases()` — matching algorithm with `ReverseInsertPts` for code motion.
- **Swift** `GlobalARCSequenceDataflow.h:33-113`: Region-based analysis with loop summarization. `ARCRegionState` tracks per-region state.

**Depends on:** Section 01 (statistics), Section 02 (barrier information), Section 03 (KnownSafe flags).

**Recommended subsection implementation order:** 05.1 → 05.5 → 05.2 → 05.3 → 05.4 → 05.6. Rationale: 05.5 (CFG hazard detection / path counting) must be available before 05.2 and 05.3 can populate the `cfg_hazard` flag, and 05.4 consumes that flag. Alternatively, integrate path-count logic directly into the BU/TD traversals (05.2/05.3), which eliminates 05.5 as a separate step.

**Data flow from dependencies:**
- **Section 01**: `SynergyMetrics` fields for tracking `rc_pairs_eliminated` / `rc_ops_moved`.
- **Section 02**: `callee_may_observe_rc()` function from `coalesce/mod.rs`, and `MemoryContract` map from `config.contracts` in `run_aims_pipeline()`.
- **Section 03**: `KnownSafeAnalysis` results — the RC motion pass must receive the `KnownSafeAnalysis` struct computed in Section 03 (stored as a local in `run_aims_pipeline()` between the Section 03 pass and this pass). The `VarRcInfo::known_safe` flag is read during pair matching (05.4).

---

## 05.1 Define Per-Block RC State Model

**File(s):** `compiler/ori_arc/src/aims/rc_motion/mod.rs` (new module)

Define the state tracked per basic block and per variable during bidirectional traversal.

- [ ] Create `compiler/ori_arc/src/aims/rc_motion/` directory

- [ ] Add `pub mod rc_motion;` to `compiler/ori_arc/src/aims/mod.rs` (alongside the existing `pub mod knownsafe;` from Section 03)

- [ ] Define `RcSequence` (analogous to LLVM's `Sequence` enum):
  ```rust
  /// State of an RC operation sequence for a single variable.
  #[derive(Clone, Copy, Debug, PartialEq, Eq)]
  pub enum RcSequence {
      /// No RC operation seen yet.
      None,
      /// RcInc seen (bottom-up: potential release match point).
      Inc,
      /// An instruction that might alter the variable's refcount was seen.
      CanRelease,
      /// A use of the variable was seen (requires positive refcount).
      Use,
      /// Sequence is stopped (can't optimize further).
      Stop,
      /// A movable RcDec was seen (can potentially be moved).
      MovableDec,
  }
  ```

- [ ] Define `VarMotionState`:
  ```rust
  /// Per-variable state for RC code motion analysis.
  #[derive(Clone, Debug)]
  pub struct VarMotionState {
      /// Current position in the retain/release sequence.
      pub seq: RcSequence,
      /// KnownSafe from Section 03 — refcount known positive.
      pub known_safe: bool,
      /// CFG hazard — motion is unsafe across this point.
      pub cfg_hazard: bool,
      /// Insertion points for the reverse direction's operations.
      pub reverse_insert_pts: SmallVec<[ArcBlockId; 2]>,
      /// The RC instructions that form this pair.
      pub pair_insts: SmallVec<[(ArcBlockId, usize); 2]>,
  }
  ```

- [ ] Define `BlockMotionState` (analogous to LLVM's `BBState`):
  ```rust
  pub struct BlockMotionState {
      /// Top-down path count (from function entry to this block).
      pub td_path_count: u32,
      /// Bottom-up path count (from this block to function exits).
      pub bu_path_count: u32,
      /// Per-variable top-down state.
      pub top_down: FxHashMap<ArcVarId, VarMotionState>,
      /// Per-variable bottom-up state.
      pub bottom_up: FxHashMap<ArcVarId, VarMotionState>,
  }
  ```

- [ ] Implement merge operations:
  - `init_from_pred()` / `merge_pred()` for top-down (analogous to LLVM `BBState::InitFromPred/MergePred`)
  - `init_from_succ()` / `merge_succ()` for bottom-up
  - Path count overflow detection (set to `OVERFLOW_VALUE = 0xFFFF_FFFF`)

- [ ] Unit tests for state model and merging.

- [ ] **Subsection close-out (05.1)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-05.1 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 05.1: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 05.2 Implement Bottom-Up Pass

**File(s):** `compiler/ori_arc/src/aims/rc_motion/bottom_up.rs` (new file)

Walk the CFG in reverse post-order, bottom-up, tracking release sequences.

**Reuse existing infrastructure:** `compiler/ori_arc/src/graph/mod.rs` provides `compute_postorder()` (post-order traversal), `compute_predecessors()`, and dominator tree construction. Reverse the postorder result to get reverse post-order (RPO). Use these rather than reimplementing CFG traversal.

- [ ] Implement `visit_bottom_up()`:
  ```rust
  /// Bottom-up pass: walk from exits to entry, tracking RcDec sequences.
  ///
  /// For each block (in reverse post-order):
  /// 1. Merge bottom-up state from all successors
  /// 2. Walk instructions backward
  /// 3. When RcDec seen: init sequence
  /// 4. When RcInc seen: try to match with existing sequence → record pair
  /// 5. When use/call seen: update sequence state (Use, CanRelease, Stop)
  pub fn visit_bottom_up(
      func: &ArcFunction,
      states: &mut FxHashMap<ArcBlockId, BlockMotionState>,
      contracts: &FxHashMap<Name, MemoryContract>,
  ) -> Vec<RcPair>
  ```

- [ ] Handle instruction effects using Section 02's barrier logic:
  - `Apply`/`ApplyIndirect`: consult `MemoryContract` for selective barrier
  - `IsShared`: use of variable (must have positive refcount)
  - `Project`: use of source variable
  - `Set`: use + potential refcount observation

- [ ] Bail out if per-block state grows too large (LLVM's `MaxPtrStates = 4095`).

- [ ] Unit tests (TDD: write BEFORE implementing `visit_bottom_up()` -- tests must fail initially): linear block, diamond CFG, loop (verifies back-edge handling), irreducible CFG (verifies conservative behavior).

- [ ] **TPR checkpoint** — `/tpr-review` covering 05.1–05.2 implementation work

- [ ] **Subsection close-out (05.2)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-05.2 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 05.2: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 05.3 Implement Top-Down Pass

**File(s):** `compiler/ori_arc/src/aims/rc_motion/top_down.rs` (new file)

Walk the CFG in post-order, top-down, tracking retain sequences.

- [ ] Implement `visit_top_down()`:
  ```rust
  /// Top-down pass: walk from entry to exits, tracking RcInc sequences.
  ///
  /// Mirror of bottom-up: when RcInc seen, init sequence.
  /// When RcDec seen, try to match. Use/call updates state.
  pub fn visit_top_down(
      func: &ArcFunction,
      states: &mut FxHashMap<ArcBlockId, BlockMotionState>,
      contracts: &FxHashMap<Name, MemoryContract>,
  ) -> Vec<RcPair>
  ```

- [ ] Use same barrier logic as bottom-up (Section 02 integration).

- [ ] Unit tests (TDD: write BEFORE implementing `visit_top_down()` -- mirror of bottom-up tests).

- [ ] **Subsection close-out (05.3)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-05.3 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 05.3: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 05.4 Pair Up and Place

**File(s):** `compiler/ori_arc/src/aims/rc_motion/placement.rs` (new file)

Match bottom-up pairs with top-down pairs and compute optimal placement.

**Execution order dependency:** This subsection consumes `cfg_hazard` flags from `VarMotionState` (condition 5 below). These flags must be set by 05.5's `check_cfg_hazards()` BEFORE `pair_up_and_place()` runs. Implementation order: **05.1 → 05.5 → 05.2 → 05.3 → 05.4 → 05.6**. Alternatively, integrate path-count computation into the BU/TD passes (05.2/05.3) directly, which is what LLVM does -- path counts are accumulated during the dataflow traversal, not in a separate pass.

- [ ] Implement `pair_up_and_place()`:
  ```rust
  /// Match retain/release pairs from both passes and compute placement.
  ///
  /// A pair is eliminable when:
  /// 1. Bottom-up found a release matching a retain (same variable)
  /// 2. Top-down found a retain matching a release (same variable)  
  /// 3. Both agree on the pair
  /// 4. KnownSafe is true (from Section 03) OR both paths are safe
  /// 5. CFGHazardAfflicted is false (path count check passes)
  ///
  /// For eliminable pairs: remove both instructions.
  /// For movable-but-not-eliminable: compute insertion points and move.
  pub fn pair_up_and_place(
      func: &mut ArcFunction,
      bu_pairs: &[RcPair],
      td_pairs: &[RcPair],
      states: &FxHashMap<ArcBlockId, BlockMotionState>,
  ) -> usize  // pairs eliminated or moved
  ```

- [ ] Movement rules:
  - Inc can move *down* toward its matching Dec (delay retain)
  - Dec can move *up* toward its matching Inc (early release)
  - Neither can move through a loop back-edge (path count mismatch)
  - Neither can move past a use that requires positive refcount

- [ ] Unit tests (TDD: write BEFORE implementing `pair_up_and_place()`): elimination in diamond, movement in triangle, loop preservation (back-edge blocks motion).

- [ ] **Subsection close-out (05.4)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-05.4 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 05.4: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 05.5 CFG Hazard Detection

**File(s):** `compiler/ori_arc/src/aims/rc_motion/hazards.rs` (new file)

Implement path-count-based CFG hazard detection (from LLVM's `BBState`).

- [ ] Implement `check_cfg_hazards()`:
  ```rust
  /// Check for CFG structures where moving RC ops would change execution count.
  ///
  /// Uses path counting: TopDownPathCount × BottomUpPathCount gives the
  /// number of unique paths through a block. If moving an RC op would
  /// cause it to execute on more paths than intended, the motion is unsafe.
  ///
  /// Overflow handling: if path count exceeds u32::MAX, fall back to
  /// conservative (no motion for that block).
  pub fn check_cfg_hazards(
      func: &ArcFunction,
      states: &mut FxHashMap<ArcBlockId, BlockMotionState>,
  )
  ```

- [ ] Compute path counts:
  - Entry block: `td_path_count = 1`
  - Exit blocks: `bu_path_count = 1`
  - Other blocks: sum of predecessor/successor path counts
  - Overflow → mark as `OVERFLOW` and clear all states for that block

- [ ] Mark `cfg_hazard = true` on `VarMotionState` when path counts indicate motion is unsafe:
  - Loop back-edge detected (successor has lower RPO than current block)
  - Path count product overflows
  - Not all successors have same sequence state (asymmetric CFG)

- [ ] Unit tests:
  - Linear CFG → no hazards
  - Diamond CFG → no hazards
  - Loop → hazard on back-edge
  - Irreducible CFG → conservative (all hazards)

- [ ] **TPR checkpoint** — `/tpr-review` covering 05.3–05.5 implementation work

- [ ] **Subsection close-out (05.5)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-05.5 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 05.5: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 05.6 Integration and Matrix Testing

**File(s):** `compiler/ori_arc/src/pipeline/aims_pipeline.rs`, tests

Integrate the RC motion pass into the AIMS pipeline.

- [ ] Insert RC motion pass into `run_aims_pipeline()`:
  - After KnownSafe pass (Section 03) and before `verify_and_merge()` which contains steps 6-9 (verify, AIMS-verify, tail calls, unwind cleanup, merge_blocks)
  - This ensures RC motion operates on the pre-merge IR with valid block structure
  - RC motion modifies RC instructions (removes/moves `RcInc`/`RcDec`), so `verify()` (step 6 inside `verify_and_merge`) will validate the modified IR. No separate re-verification needed since `verify_and_merge` runs after RC motion.

  **Data threading:** The RC motion pass needs:
  1. `&ArcFunction` — the post-emission IR
  2. `contracts: &FxHashMap<Name, MemoryContract>` — from `config.contracts` (for `callee_may_observe_rc()`)
  3. `analysis: &KnownSafeAnalysis` — from Section 03's pass (stored as a local between the KnownSafe and RC motion invocations in `run_aims_pipeline()`)
  4. `metrics: &mut SynergyMetrics` — to record `rc_pairs_eliminated` and `rc_ops_moved`

  **Prerequisite: `aims_pipeline.rs` splitting** (see overview Prerequisites table). The pipeline file is 590 lines — at the 500-line limit. This extraction must be done before adding RC motion invocation. If Section 03 or 04 is implemented first, they will have already completed this.

- [ ] Add `rc_pairs_eliminated` and `rc_ops_moved` to `SynergyMetrics`

- [ ] **Matrix testing:**
  - **Type dimension**: str, [int], Option\<str\>, closures, maps, nested structs
  - **CFG dimension**: linear, diamond, triangle, loop (while), loop (for), nested loop, match arms, early return, try/catch
  - **RC pattern dimension**: retain-release in same block (no motion needed), retain in pred + release in succ (motion across diamond), retain outside loop + release inside (should NOT move), nested function calls

- [ ] **Correctness verification:**
  - `ORI_CHECK_LEAKS=1` on all test programs → zero leaks
  - `ORI_TRACE_RC=1` comparison before/after → same logical RC balance
  - Dual-exec parity: interpreter and LLVM produce identical results
  - Debug and release builds produce identical results

- [ ] **Measurement:**
  - `ORI_LOG=ori_arc=info ori build tests/spec/` → report `rc_pairs_eliminated`, `rc_ops_moved`
  - Compare total RC operations before/after (using Section 01 statistics)
  - Target: 10-25% additional RC reduction on programs with cross-block RC patterns

- [ ] **Stress testing:**
  - Programs with 100+ basic blocks
  - Deeply nested control flow (10+ levels)
  - Programs with complex loop nesting
  - Verify no exponential blowup in analysis time

- [ ] **Verify tests pass in debug and release**

**Semantic pin:** Test that a diamond CFG with `RcInc(x)` in the entry block and `RcDec(x)` in both successors (same variable, no intervening use in the merge block) results in `rc_pairs_eliminated > 0`. This pattern is only optimizable with cross-block motion -- single-block passes cannot see it.

**Negative pin:** Test that a loop containing `RcInc(x)` in the body does NOT get its inc moved outside the loop (path count mismatch would change execution count). Assert `rc_ops_moved == 0` for a loop-body-only RC pattern. A second negative pin: test that a CFG with a use between the inc and dec that requires positive refcount does NOT have the pair eliminated -- the use would fault if the pair were removed.

- [ ] **TPR checkpoint** -- `/tpr-review` covering 05.6 integration and full Section 05 implementation

- [ ] **Subsection close-out (05.6)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-05.6 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 05.6: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 05.R Third Party Review Findings

- None.

---

## 05.N Completion Checklist

- [ ] `RcSequence` state machine with correct transitions
- [ ] `BlockMotionState` with path counting and overflow detection
- [ ] Bottom-up pass correctly identifies release sequences
- [ ] Top-down pass correctly identifies retain sequences
- [ ] `pair_up_and_place()` correctly matches and eliminates/moves pairs
- [ ] CFG hazard detection prevents motion through loop back-edges
- [ ] Path count overflow → conservative (no motion)
- [ ] Section 02 barrier information used for selective interference queries
- [ ] Section 03 KnownSafe flags consumed correctly
- [ ] `rc_pairs_eliminated` and `rc_ops_moved` tracked in `SynergyMetrics`
- [ ] No exponential blowup on large CFGs (bail out at `MaxPtrStates`)
- [ ] `ORI_CHECK_LEAKS=1` reports zero leaks on all test programs
- [ ] `ORI_TRACE_RC=1` shows balanced RC counts before/after
- [ ] Dual-exec parity maintained (interpreter == LLVM)
- [ ] `./test-all.sh` green (debug + release)
- [ ] No spurious warnings in normal compilation
- [ ] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan 05` returns 0 annotations
- [ ] All intermediate TPR checkpoint findings resolved
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues
- [ ] `/impl-hygiene-review` passed — hygiene review clean
- [ ] `/improve-tooling` retrospective completed — MANDATORY at section close, after both reviews are clean. Reflect on the section's debugging journey (which `diagnostics/` scripts you ran, which command sequences you repeated, where you added ad-hoc `dbg!`/`tracing` calls, where output was hard to interpret) and identify any tool/log/diagnostic improvement that would have made this section materially easier OR that would help the next section touching this area. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. The retrospective is mandatory even when nothing felt painful — that is exactly when blind spots accumulate. See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode" for the full protocol.

**Exit Criteria:** `ORI_LOG=ori_arc=info ori build` shows `rc_pairs_eliminated > 0` on programs with cross-block RC patterns (e.g., retain in one branch, release in another of a diamond). CFG hazard detection correctly prevents motion through loops. All existing tests pass. `ORI_CHECK_LEAKS=1` reports zero leaks. Analysis completes in O(n × vars × blocks) time without exponential blowup.
