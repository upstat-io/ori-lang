---
section: "05"
title: "AIMS Pass Bisection"
status: not-started
reviewed: false
goal: "Create bisect-passes.sh that automatically identifies which AIMS pipeline phase introduced a failure or RC imbalance"
success_criteria:
  - "Per-phase trace checkpoints emit after each of the 12 top-level AIMS pipeline steps (not just post-walk subphases)"
  - "bisect-passes.sh runs a program, captures per-phase snapshots, and reports which phase first introduces a failure or RC divergence"
  - "Output shows: phase name, before/after RC operation counts, and whether the program still produces correct output after that phase"
inspired_by:
  - "Swift sil-opt-pass-count — bisects optimization passes to find the one that introduced a bug"
  - "Lean 4 trace.compiler.ir.rc — per-phase RC tracing"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "05.1"
    title: "Add per-phase trace checkpoints to AIMS pipeline"
    status: not-started
  - id: "05.2"
    title: "Create bisect-passes.sh shell driver"
    status: not-started
  - id: "05.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "05.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 05: AIMS Pass Bisection

**Status:** Not Started
**Goal:** When a program crashes, leaks, or produces wrong output through the AOT pipeline, `bisect-passes.sh` answers "which AIMS pipeline phase broke it?" Currently this requires manual trace-log spelunking with `ORI_LOG=ori_arc::aims::realize=trace` — slow, error-prone, and expertise-dependent. The existing `trace_phase_snapshot` only covers post-walk realization subphases (step 5 substeps), not all 12 top-level pipeline steps.

**Success Criteria:**
- [ ] All 12 AIMS pipeline phases emit a named checkpoint to tracing with per-phase RC operation counts
- [ ] `bisect-passes.sh file.ori` compiles once with full tracing, parses the per-phase checkpoint events, and reports which phase first changed the RC balance (e.g., "realize_rc_reuse added +3 RcInc, +2 RcDec; realize_annotations added +1 RcDec — net balance went from 0 to -1 here")
- [ ] Output is a human-readable table: Phase | RcInc | RcDec | Balance | Delta-from-previous
- [ ] The script does NOT claim to execute intermediate phase results — it analyzes the tracing output from a single full compilation to identify where RC divergence first appears
- [ ] Satisfies mission criterion: "bisect-passes.sh can identify which AIMS pipeline phase introduced a failure"

**Scope clarification:** This is a **trace-analysis bisector**, not a stop-after-phase executor. The AIMS pipeline runs all 12 phases in a single compilation. The shell driver analyzes the per-phase RC snapshots to find where the balance first diverges. It cannot execute the program at intermediate pipeline states — that would require a stop-after-phase compiler surface which is out of scope for this plan. The trace-analysis approach is still enormously valuable: it answers "which phase changed the RC balance" without manual log spelunking.

**Context:** Both Codex and Gemini flagged the absence of automated AIMS bisection. Codex's analysis: existing `trace_phase_snapshot` in `emit_unified.rs` only covers the post-walk realization subpipeline (after step 5), not the 12 top-level steps. Gemini referenced Swift's `sil-opt-pass-count` as the model. The implementation requires a small Rust change (adding checkpoints) plus a shell driver.

**Reference implementations:**
- **Swift** `sil-opt-pass-count` — bisects SIL optimization passes (docs/DebuggingTheCompiler.md)
- **Lean 4** `trace.compiler.ir.rc` — per-phase RC tracing with configurable granularity

**Depends on:** None.

---

## 05.1 Add per-phase trace checkpoints to AIMS pipeline

**File(s):** `compiler/ori_arc/src/pipeline/aims_pipeline.rs`

The AIMS pipeline orchestration runs 12 steps in `run_aims_pipeline()`. Add a tracing event after each step that logs the phase name and a summary of the function's current state (total RC ops, block count, var count).

- [ ] Find the pipeline orchestration: `compiler/ori_arc/src/pipeline/aims_pipeline.rs` — the `run_aims_pipeline()` or `run_arc_pipeline()` function
- [ ] After each of the 12 top-level steps, add a tracing event:
  ```rust
  tracing::info!(
      target: "ori_arc::aims::pipeline",
      phase = "compute_var_reprs",
      rc_incs = %count_rc_incs(&func),
      rc_decs = %count_rc_decs(&func),
      blocks = %func.blocks.len(),
      "AIMS phase checkpoint"
  );
  ```
  The `count_rc_incs`/`count_rc_decs` helpers count `ArcInstr::RcInc`/`RcDec` in the function. These may already exist or be trivially derivable.
- [ ] Use `info` level (not `debug` or `trace`) so the events are captured by the shell driver without overwhelming verbosity
- [ ] Add a Rust unit test verifying that the checkpoint events fire (use `tracing-test` subscriber if available, or check that the function doesn't panic with tracing enabled)
- [ ] Verify: `ORI_LOG=ori_arc::aims::pipeline=info ori build diagnostics/fixtures/clean.ori 2>&1 | grep "AIMS phase checkpoint"` shows 12 checkpoint lines
- [ ] Run `timeout 150 cargo t -p ori_arc` to verify no regressions

**File size check:** `aims_pipeline.rs` is currently 590 lines (already above the 500-line limit). Adding checkpoints will increase it further. Before adding checkpoints:
- [ ] Extract the checkpoint/tracing helper into a small utility function (e.g., `fn trace_pipeline_checkpoint(func: &ArcFunction, phase: &str)`) — this keeps the checkpoint code out of the main pipeline flow
- [ ] If `aims_pipeline.rs` exceeds 600 lines after adding checkpoints, extract the pipeline configuration or helper functions into a submodule

- [ ] **Subsection close-out (05.1)** — MANDATORY before starting 05.2:
  - [ ] All tasks above are `[x]` and verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 05.2 Create bisect-passes.sh shell driver

**File(s):** `diagnostics/bisect-passes.sh` (new), `diagnostics/self-test.sh`, `diagnostics/README.md`

Create a shell script that runs a program with `ORI_LOG=ori_arc::aims::pipeline=info`, parses the checkpoint events, and reports which phase first introduces a problem.

- [ ] Create `diagnostics/bisect-passes.sh` with:
  - `--help`, `--no-color`, `--color` (standard options)
  - `--function <name>` — filter to a specific function's phases
  - Input: a single `.ori` file
  - Behavior:
    1. Compile with `ORI_LOG=ori_arc::aims::pipeline=info` and capture stderr
    2. Parse checkpoint events to extract per-phase RC counts (phase name, rc_incs, rc_decs, blocks)
    3. Display a table: `Phase | RC Incs | RC Decs | Balance | Delta`
    4. Highlight the first phase where balance changes from 0 to non-zero (potential leak/over-release introduction)
    5. Also run the binary and check: does it crash? Does `ORI_CHECK_LEAKS=1` report leaks?
  - Exit codes: 0 = all phases clean, 1 = divergence detected, 2 = usage/infrastructure error
  - Source `_common.sh` for binary discovery
- [ ] Add self-test entries:
  - `bisect-passes.sh --help` shows usage
  - `bisect-passes.sh fixtures/simple.ori` runs without error
  - `bisect-passes.sh fixtures/clean.ori` shows phase table with non-trivial RC counts
- [ ] Add documentation to `diagnostics/README.md`
- [ ] Verify: `diagnostics/self-test.sh` passes

- [ ] **Subsection close-out (05.2)** — MANDATORY before starting 05.R:
  - [ ] All tasks above are `[x]` and verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 05.R Third Party Review Findings

- None.

---

## 05.N Completion Checklist

- [ ] All subsections (05.1, 05.2) complete
- [ ] `timeout 150 cargo t -p ori_arc` passes
- [ ] `diagnostics/self-test.sh` passes
- [ ] `timeout 150 ./test-all.sh` green — no regressions
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review` passed
- [ ] **`/improve-tooling` section-close sweep**
