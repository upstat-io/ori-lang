---
section: "01"
title: "Compile-Time ARC Statistics"
status: not-started
reviewed: true
goal: "Every AIMS pipeline pass reports before/after RC operation counts via tracing, enabling measurable optimization development"
inspired_by:
  - "LLVM ObjCARCOpts.cpp STATISTIC() macro pattern (llvm-project/llvm/lib/Transforms/ObjCARC/ObjCARCOpts.cpp:159-175)"
  - "Swift ARCSequenceOpts.cpp NumRefCountOpsRemoved (swift/lib/SILOptimizer/ARC/ARCSequenceOpts.cpp:40)"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "01.1"
    title: "Extend SynergyMetrics with RC Operation Counts"
    status: not-started
  - id: "01.2"
    title: "Add Pipeline-Level Before/After Instrumentation"
    status: not-started
  - id: "01.3"
    title: "Add Coalescing-Specific Statistics"
    status: not-started
  - id: "01.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "01.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 01: Compile-Time ARC Statistics

**Status:** Not Started
**Goal:** Every AIMS pipeline pass reports before/after RC operation counts via `tracing::info!`, providing measurable feedback for optimization development. Running `ORI_LOG=ori_arc=info ori build file.ori` shows RC operations inserted, coalesced, and eliminated per function.

**Context:** LLVM's ARC optimizer uses the `STATISTIC()` macro extensively — tracking `NumRRs` (retain/release pairs eliminated), `NumNoops` (no-op calls removed), and debug-only `NumRetainsBeforeOpt/NumRetainsAfterOpt` counts. Ori's `SynergyMetrics` already tracks cross-dimensional evidence but lacks the most basic measurement: how many RC operations were emitted vs how many survived to final output. Without these numbers, optimization work in Sections 02-05 cannot prove its value.

**Reference implementations:**
- **LLVM** `llvm/lib/Transforms/ObjCARC/ObjCARCOpts.cpp:159-175`: Fine-grained per-pattern counters with debug-only before/after counts. Each counter maps to a distinct optimization rule.
- **Swift** `swift/lib/SILOptimizer/ARC/ARCSequenceOpts.cpp:40`: Single aggregate `NumRefCountOpsRemoved`. Simpler but less useful for debugging.

**Depends on:** None (pure additive instrumentation).

---

## 01.1 Extend SynergyMetrics with RC Operation Counts

**File(s):** `compiler/ori_arc/src/aims/realize/metrics.rs`

Add before/after RC operation counts to `SynergyMetrics`. These complement the existing cross-dimensional evidence tracking with the most fundamental measurement: raw RC operation volume.

- [ ] Add fields to `SynergyMetrics`:
  ```rust
  /// RC operations in input IR (before any AIMS processing).
  pub rc_ops_input: usize,
  /// RC operations after Phase 1 realization (post-emission, pre-coalesce).
  pub rc_ops_post_emission: usize,
  /// RC operations after coalescing.
  pub rc_ops_post_coalesce: usize,
  /// Barrier flushes during coalescing (total flush_all + flush_touched calls).
  pub barrier_flush_count: usize,
  /// Full-barrier flushes (flush_all at Apply/ApplyIndirect).
  pub barrier_flush_all_count: usize,
  ```

- [ ] Add `coalesce_reduction_percent()` method:
  ```rust
  pub fn coalesce_reduction_percent(&self) -> f64 {
      if self.rc_ops_post_emission == 0 { return 0.0; }
      let reduced = self.rc_ops_post_emission.saturating_sub(self.rc_ops_post_coalesce);
      (reduced as f64 / self.rc_ops_post_emission as f64) * 100.0
  }
  ```

- [ ] Update `report()` to include new fields:
  ```rust
  tracing::info!(
      // ... existing fields ...
      rc_ops_input = self.rc_ops_input,
      rc_ops_post_emission = self.rc_ops_post_emission,
      rc_ops_post_coalesce = self.rc_ops_post_coalesce,
      coalesce_reduction = format_args!("{:.1}%", self.coalesce_reduction_percent()),
      barrier_flushes = self.barrier_flush_count,
      barrier_flush_all = self.barrier_flush_all_count,
      "AIMS synergy metrics"
  );
  ```

- [ ] Update `merge()` to include new fields (additive).

- [ ] Unit tests in `realize/tests.rs`:
  - Test `coalesce_reduction_percent()` with zero, partial, and full reduction
  - Test `merge()` with new fields

- [ ] **Subsection close-out (01.1)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-01.1 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 01.1: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 01.2 Add Pipeline-Level Before/After Instrumentation

**File(s):** `compiler/ori_arc/src/pipeline/aims_pipeline.rs`, `compiler/ori_arc/src/aims/realize/mod.rs`

Count RC operations at each pipeline stage by scanning the IR instruction lists.

- [ ] Reuse existing `count_rc_ops()` from `pipeline/rc_count/mod.rs`:
  ```rust
  // Already exists in pipeline/rc_count/mod.rs:
  // pub(crate) fn count_rc_ops(func: &ArcFunction) -> RcOpCount
  // Returns RcOpCount { inc, dec } with inc summing batched counts.
  // Use .total() for the combined inc+dec count.
  ```

- [ ] **Fix hygiene: eliminate duplicate `count_rc_ops()`** — `realize/emit_unified.rs:242` has a private `count_rc_ops()` that returns `usize` (simple count without batching). The pipeline version in `pipeline/rc_count/mod.rs` returns `RcOpCount { inc, dec }` with batched `RcInc` count handling (respects `count > 1`). Replace the private copy in `emit_unified.rs` with `crate::pipeline::rc_count::count_rc_ops(func).total()` and delete the private function. This fixes a LEAK:algorithmic-duplication finding.

- [ ] Instrument the pipeline — two separate sites:

  **Site 1: `pipeline/aims_pipeline.rs`** (per-function pipeline):
  - Before step 4 (`analyze_function`): `metrics.rc_ops_input = count_rc_ops(func).total()` — this will be 0 for input IR (RC ops haven't been emitted yet), serving as the baseline.
  - After step 5 (`realize_rc_reuse`) returns: capture `rc_ops_post_coalesce` via `count_rc_ops(func).total()`. Note: `realize_rc_reuse` internally calls `emit_rc_unified` which calls `coalesce_block_rc`, so by the time step 5 returns, coalescing has already happened. This count IS the post-coalesce count, not pre-coalesce.

  **Site 2: `realize/emit_unified.rs`** (inside `emit_rc_unified()`):
  - Capture `rc_ops_post_emission` (pre-coalesce) count BEFORE the Phase 3 coalescing loop at line ~106. After the hygiene fix above, use `crate::pipeline::rc_count::count_rc_ops(func).total()`.
  - Return the pre-coalesce count from `emit_rc_unified()` alongside the existing return tuple (add it as a 5th element or add it to the `SynergyMetrics` directly).
  - The coalescing loop at line ~107 then runs, after which `count_rc_ops(func).total()` at line ~111 gives the post-coalesce count.
  - Both counts must reach `SynergyMetrics`.

- [ ] Verify with: `ORI_LOG=ori_arc=info ori build tests/spec/traits/iterator/map.ori` shows per-function metrics.

**Matrix testing:**
- **Types**: int (scalar, no RC), str (heap), [int] (collection), Option\<str\> (enum+heap), closures
- **Patterns**: simple function, loop with RC, nested calls, COW mutation

**Semantic pin:** Test that counts str and [int] programs as having `rc_ops_post_emission > 0` and int-only programs as having `rc_ops_post_emission == 0`.

**Negative pin:** Test that `rc_ops_post_emission` for a program containing only `let $x = 42; x + 1` is exactly 0 — rejects the possibility that the instrumentation introduces spurious non-zero counts for scalar-only code. If this test ever passes with a non-zero value, the counting logic is wrong.

- [ ] **Subsection close-out (01.2)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-01.2 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 01.2: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 01.3 Add Coalescing-Specific Statistics

**File(s):** `compiler/ori_arc/src/aims/emit_rc/coalesce/mod.rs`

Thread metrics through the coalescing pass to count barrier events.

- [ ] Add `CoalesceStats` struct:
  ```rust
  pub(crate) struct CoalesceStats {
      pub flush_all_count: usize,
      pub flush_touched_count: usize,
      pub pairs_cancelled: usize,  // inc+dec pairs that netted to zero
      pub ops_merged: usize,       // adjacent inc/inc or dec/dec merged
  }
  ```

- [ ] Update `coalesce_block_rc()` signature to return `CoalesceStats`:
  ```rust
  pub(crate) fn coalesce_block_rc(body: &mut Vec<ArcInstr>) -> CoalesceStats
  ```

- [ ] Count events inside the coalescing loop:
  - Increment `flush_all_count` at the `flush_all()` call inside the `if is_call` branch (line 74 of `coalesce/mod.rs`)
  - Increment `flush_touched_count` at the `flush_touched()` call in the else branch (line 77)
  - In `flush_entry()` (line 161), count `pairs_cancelled` when `incs == decs` (net zero — neither the `> entry.decs` nor `> entry.incs` branch fires, so both are cancelled)
  - In `flush_entry()`, count `ops_merged` when `incs > 1` or `decs > 1` (the emitted op represents a merge of multiple individual ops)

  **Note:** `flush_entry()` currently takes `&PendingRc`, not `&mut CoalesceStats`. Either pass `&mut CoalesceStats` to `flush_entry()`, or count at the call sites in `flush_all()` and `flush_touched()` after checking the entry values before removal.

- [ ] Propagate `CoalesceStats` up through the call chain:
  1. `coalesce_block_rc()` returns `CoalesceStats` (already changed above)
  2. `emit_rc_unified()` in `realize/emit_unified.rs` calls `coalesce_block_rc()` per block at line ~108 — accumulate per-block stats into a function-level aggregate
  3. `emit_rc_unified()` returns the aggregate alongside existing return tuple (extend the return type or embed in `SynergyMetrics`)
  4. `realize_rc_reuse()` in `realize/mod.rs` calls `emit_rc_unified()` — propagate to `SynergyMetrics`:
  ```rust
  metrics.barrier_flush_count = stats.flush_all_count + stats.flush_touched_count;
  metrics.barrier_flush_all_count = stats.flush_all_count;
  ```

- [ ] Unit tests:
  - Test that a block with no calls has `flush_all_count == 0`
  - Test that a block with 3 Apply instructions has `flush_all_count == 3`
  - Test that `RcInc(x); RcDec(x)` with no barrier has `pairs_cancelled == 1`

**Semantic pin:** Test that `flush_all_count` equals the number of Apply/ApplyIndirect instructions in a block (before Section 02 changes the barrier behavior).

**Negative pin:** Test that `pairs_cancelled` for a block containing `RcInc(x); Apply(f, x); RcDec(x)` is exactly 0 — the intervening call barrier prevents cancellation. If this ever reports `pairs_cancelled > 0`, the barrier-respecting logic in coalescing is broken.

- [ ] **Subsection close-out (01.3)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-01.3 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 01.3: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 01.R Third Party Review Findings

- None.

---

## 01.N Completion Checklist

- [ ] `SynergyMetrics` has `rc_ops_input`, `rc_ops_post_emission`, `rc_ops_post_coalesce`, `barrier_flush_count`, `barrier_flush_all_count` fields
- [ ] `coalesce_block_rc()` returns `CoalesceStats` with `flush_all_count`, `flush_touched_count`, `pairs_cancelled`, `ops_merged`
- [ ] `ORI_LOG=ori_arc=info ori build tests/spec/traits/iterator/map.ori` shows per-function RC metrics
- [ ] Scalar-only programs show `rc_ops_post_emission == 0`
- [ ] Programs with heap types show `rc_ops_post_emission > 0`
- [ ] `coalesce_reduction_percent()` returns correct values for test cases
- [ ] Duplicate `count_rc_ops()` in `emit_unified.rs` deleted — single canonical version in `pipeline/rc_count/mod.rs`
- [ ] All existing tests pass unchanged (`./test-all.sh` green)
- [ ] No spurious warnings in normal compilation
- [ ] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan 01` returns 0 annotations
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues
- [ ] `/impl-hygiene-review` passed — hygiene review clean
- [ ] `/improve-tooling` retrospective completed — MANDATORY at section close, after both reviews are clean. Reflect on the section's debugging journey (which `diagnostics/` scripts you ran, which command sequences you repeated, where you added ad-hoc `dbg!`/`tracing` calls, where output was hard to interpret) and identify any tool/log/diagnostic improvement that would have made this section materially easier OR that would help the next section touching this area. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. The retrospective is mandatory even when nothing felt painful — that is exactly when blind spots accumulate. See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode" for the full protocol.

**Exit Criteria:** `ORI_LOG=ori_arc=info ori build` on any `.ori` file emits structured tracing events with `rc_ops_input`, `rc_ops_post_emission`, `rc_ops_post_coalesce`, `barrier_flush_count` fields. `./test-all.sh` passes with 0 regressions. `CoalesceStats` counts match manual inspection of IR for 3+ test programs.
