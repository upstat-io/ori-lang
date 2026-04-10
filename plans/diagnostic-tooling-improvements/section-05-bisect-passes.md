---
section: "05"
title: "AIMS Pass Bisection"
status: not-started
reviewed: false
goal: "Create bisect-passes.sh that automatically identifies which AIMS pipeline phase introduced an RC imbalance or structural change"
success_criteria:
  - "Mandatory file split: aims_pipeline.rs extracted into submodules BEFORE any checkpoint code is added (file is 590 lines, above the 500-line limit)"
  - "Per-phase trace checkpoints emit after each logical AIMS pipeline step — including inside helpers (normalize_with_trmc, verify_and_merge, emit_postprocess) at each sub-step boundary"
  - "Checkpoints use existing rc_count::count_rc_ops() (returns RcOpCount { inc, dec }) — no new count_rc_incs/count_rc_decs helpers"
  - "Checkpoints emit RC counts AND structural metrics (block count, var count) to detect phases that change behavior without changing RC totals"
  - "bisect-passes.sh runs a program, captures per-phase snapshots, and reports which phase first introduces an RC divergence or structural change"
  - "Output shows: phase name, RC incs, RC decs, balance, block count, var count, delta-from-previous — no claim of per-phase program execution"
inspired_by:
  - "Swift sil-opt-pass-count — bisects optimization passes to find the one that introduced a bug"
  - "Lean 4 trace.compiler.ir.rc — per-phase RC tracing"
depends_on: []
third_party_review:
  status: resolved
  updated: 2026-04-10
sections:
  - id: "05.PRE"
    title: "Mandatory file split of aims_pipeline.rs"
    status: not-started
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
**Goal:** When a program crashes, leaks, or produces wrong output through the AOT pipeline, `bisect-passes.sh` answers "which AIMS pipeline phase broke it?" Currently this requires manual trace-log spelunking with `ORI_LOG=ori_arc::aims::realize=trace` -- slow, error-prone, and expertise-dependent. The existing `trace_phase_snapshot` in `emit_unified.rs` only covers post-walk realization subphases (step 5 substeps), not the top-level pipeline steps or the steps inside helper functions like `normalize_with_trmc()`, `verify_and_merge()`, and `emit_postprocess()`.

**Success Criteria:**
- [ ] `aims_pipeline.rs` (590 lines, above 500-line limit) split into submodules BEFORE any checkpoint instrumentation
- [ ] All logical AIMS pipeline phases emit a named checkpoint to tracing with per-phase RC operation counts AND structural metrics (block count, var count)
- [ ] Checkpoints fire inside helper functions at each sub-step boundary -- not just at the top-level `run_aims_pipeline()` call sites
- [ ] Checkpoints use the existing `rc_count::count_rc_ops()` helper (returns `RcOpCount { inc, dec }`) -- no new counting helpers are created
- [ ] `bisect-passes.sh file.ori` compiles once with full tracing, parses the per-phase checkpoint events, and reports which phase first changed RC balance or structural metrics
- [ ] Output is a human-readable table: Phase | RcInc | RcDec | Balance | Blocks | Vars | Delta
- [ ] The script does NOT claim to execute intermediate phase results -- it analyzes the tracing output from a single full compilation

**Scope clarification:** This is a **trace-analysis bisector**, not a stop-after-phase executor. The AIMS pipeline runs all phases in a single compilation. The shell driver analyzes the per-phase snapshots to find where the balance first diverges or structural metrics change unexpectedly. It cannot execute the program at intermediate pipeline states -- that would require a stop-after-phase compiler surface which is out of scope. The trace-analysis approach is still enormously valuable: it answers "which phase changed the RC balance" and "which phase changed the block/var structure" without manual log spelunking.

**Why structural metrics matter:** Phases like `merge_blocks()`, tail-call rewrite, and TRMC normalization change program behavior without changing RC totals. A script that only tracks RC counts has blind spots for these phases. Tracking block count and var count alongside RC counts ensures the bisector detects ALL structural changes, not just RC changes.

**Context:** Both Codex and Gemini flagged the absence of automated AIMS bisection. Codex's analysis: existing `trace_phase_snapshot` in `emit_unified.rs` only covers the post-walk realization subpipeline (after step 5), not the top-level steps or the steps inside helpers. Gemini referenced Swift's `sil-opt-pass-count` as the model. The implementation requires a small Rust change (adding checkpoints) plus a shell driver.

**Reference implementations:**
- **Swift** `sil-opt-pass-count` -- bisects SIL optimization passes (docs/DebuggingTheCompiler.md)
- **Lean 4** `trace.compiler.ir.rc` -- per-phase RC tracing with configurable granularity

**Depends on:** None.

---

## 05.PRE Mandatory file split of aims_pipeline.rs

**File(s):** `compiler/ori_arc/src/pipeline/aims_pipeline.rs` (590 lines), `compiler/ori_arc/src/pipeline/mod.rs`

Per CLAUDE.md coding guidelines: "500 line limit (excl. tests). Stop and split before exceeding." The file is already at 590 lines -- above the limit. Adding checkpoint instrumentation would push it further. The split is mandatory BEFORE any new code is added, not conditional on exceeding 600 lines.

**Split strategy:** The file contains four distinct responsibilities:
1. `AimsPipelineConfig` + `AimsPipelineResult` structs + `run_aims_pipeline()` (pipeline orchestration) -- stays in `aims_pipeline.rs`
2. `normalize_with_trmc()` + `verify_trmc_soundness()` + `detect_immortals()` (TRMC normalization loop) -- extract to `aims_pipeline/trmc.rs`
3. `verify_and_merge()` + `emit_postprocess()` + `check_fbip()` (post-emission processing) -- extract to `aims_pipeline/postprocess.rs`
4. `run_aims_pipeline_all()` + `run_second_pass()` + `apply_aims_ownership()` + `param_contract_to_ownership()` (batch orchestration + second pass) -- extract to `aims_pipeline/batch.rs`

- [ ] Convert `aims_pipeline.rs` to `aims_pipeline/mod.rs` with submodules
- [ ] Extract `normalize_with_trmc()`, `verify_trmc_soundness()`, `detect_immortals()` into `aims_pipeline/trmc.rs` (~130 lines)
- [ ] Extract `verify_and_merge()`, `emit_postprocess()`, `check_fbip()` into `aims_pipeline/postprocess.rs` (~60 lines)
- [ ] Extract `run_aims_pipeline_all()`, `run_second_pass()`, `apply_aims_ownership()`, `param_contract_to_ownership()` into `aims_pipeline/batch.rs` (~200 lines)
- [ ] Keep `AimsPipelineConfig`, `AimsPipelineResult`, `run_aims_pipeline()` in `aims_pipeline/mod.rs` (~180 lines)
- [ ] Update `mod.rs` to reference `aims_pipeline` as a directory module (should work unchanged since it already has `mod aims_pipeline;`)
- [ ] Verify: `timeout 150 cargo t -p ori_arc` passes -- no regressions from the split
- [ ] Verify: `timeout 150 cargo cl -p ori_arc` (clippy) passes
- [ ] Verify: no file in the split exceeds 300 lines

- [ ] **Subsection close-out (05.PRE)** -- MANDATORY before starting 05.1:
  - [ ] All tasks above are `[x]` and verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`

---

## 05.1 Add per-phase trace checkpoints to AIMS pipeline

**File(s):** `compiler/ori_arc/src/pipeline/aims_pipeline/mod.rs` (post-split), `compiler/ori_arc/src/pipeline/aims_pipeline/trmc.rs`, `compiler/ori_arc/src/pipeline/aims_pipeline/postprocess.rs`, `compiler/ori_arc/src/pipeline/aims_pipeline/batch.rs`

The AIMS pipeline orchestration runs steps 3-12. The top-level `run_aims_pipeline()` calls into helper functions that bundle multiple steps: `normalize_with_trmc()` bundles steps 3/3.5/3a, `verify_and_merge()` bundles steps 6/7/8/8.5/9, and `emit_postprocess()` bundles steps 11/12. Checkpoints must go INSIDE these helpers at each logical pass boundary, not just at the top-level call sites.

### 05.1.1 Create checkpoint helper

- [ ] Create a `trace_pipeline_checkpoint()` helper function (in `aims_pipeline/mod.rs` or a shared location):
  ```rust
  /// Emit a pipeline checkpoint event for bisect-passes.sh consumption.
  ///
  /// Uses `info` level on the NEW `ori_arc::aims::pipeline` target so it
  /// can be captured with `ORI_LOG=ori_arc::aims::pipeline=info` without
  /// overwhelming verbosity. Uses existing `rc_count::count_rc_ops()`.
  ///
  /// NOTE: The existing `trace_phase_snapshot` in `emit_unified.rs` uses
  /// `trace!` level on `ori_arc::aims::realize`. This helper introduces a
  /// DIFFERENT target at a DIFFERENT level for a different purpose
  /// (pipeline-level bisection vs realization-step snapshots).
  pub(crate) fn trace_pipeline_checkpoint(
      func: &ArcFunction,
      phase: &str,
      interner: &ori_ir::StringInterner,
  ) {
      use super::rc_count::count_rc_ops;
      if !tracing::enabled!(target: "ori_arc::aims::pipeline", tracing::Level::INFO) {
          return;
      }
      let fn_name = interner.lookup(func.name);
      let rc = count_rc_ops(func);
      let blocks = func.blocks.len();
      let vars = func.var_types.len();
      tracing::info!(
          target: "ori_arc::aims::pipeline",
          function = fn_name,
          phase,
          rc_incs = rc.inc,
          rc_decs = rc.dec,
          blocks,
          vars,
          "AIMS phase checkpoint"
      );
  }
  ```
  **Design notes:**
  - Accepts `interner` to resolve `func.name` to a human-readable string. This enables `bisect-passes.sh --function <name>` filtering and per-function table grouping in multi-function programs.
  - Uses `info` level on the NEW `ori_arc::aims::pipeline` target. This is intentionally different from the existing `trace_phase_snapshot` in `emit_unified.rs` which uses `trace!` on `ori_arc::aims::realize` (for finer-grained realization-step snapshots). The `info` level is chosen because checkpoint events are coarser-grained and easier for shell scripts to parse without noise.
  - Uses `count_rc_ops()` from `super::rc_count` -- the existing SSOT for RC counting.
  - Includes `blocks` and `vars` structural metrics alongside RC counts to detect phases that change CFG structure without altering RC totals.

### 05.1.2 Add checkpoints inside helpers

Checkpoints must be placed INSIDE the helper functions at each logical sub-step boundary. The following is the complete list of checkpoint locations:

All checkpoint calls pass `config.interner` (or the interner from the enclosing scope) so the function name is emitted on every event.

**Inside `normalize_with_trmc()` (trmc.rs):**
- [ ] After `compute_var_reprs()` (step 3): `trace_pipeline_checkpoint(func, "compute_var_reprs", interner)`
- [ ] After `detect_immortals()` (step 3.5): `trace_pipeline_checkpoint(func, "detect_immortals", interner)`
- [ ] After `normalize_function()` (step 3a): `trace_pipeline_checkpoint(func, "normalize_function", interner)`

**Inside `run_aims_pipeline()` (mod.rs) -- between top-level calls:**
- [ ] After `normalize_with_trmc()` returns: `trace_pipeline_checkpoint(func, "normalize_with_trmc_complete", config.interner)`
- [ ] After `analyze_function()` (step 4): `trace_pipeline_checkpoint(func, "analyze_function", config.interner)`
- [ ] After `verify_trmc_soundness()` (step 4a): `trace_pipeline_checkpoint(func, "verify_trmc_soundness", config.interner)`
- [ ] After `realize_rc_reuse()` (step 5): `trace_pipeline_checkpoint(func, "realize_rc_reuse", config.interner)`
- [ ] After FIP enforcement pre-check (step 5a): `trace_pipeline_checkpoint(func, "verify_fip_contract", config.interner)`

**Inside `verify_and_merge()` (postprocess.rs) -- needs interner parameter added:**
- [ ] After `run_verify()` (step 6): `trace_pipeline_checkpoint(func, "verify_post_emission", interner)`
- [ ] After `run_aims_verify()` (step 7): `trace_pipeline_checkpoint(func, "aims_verify", interner)`
- [ ] After `detect_tail_calls()` + `rewrite_tail_calls()` (step 8): `trace_pipeline_checkpoint(func, "tail_calls", interner)`
- [ ] After `add_invoke_unwind_cleanup()` (step 8.5): `trace_pipeline_checkpoint(func, "unwind_cleanup", interner)`
- [ ] After `merge_blocks()` (step 9): `trace_pipeline_checkpoint(func, "merge_blocks", interner)`

**Back in `run_aims_pipeline()` after `verify_and_merge()` returns:**
- [ ] After `realize_annotations()` (step 10): `trace_pipeline_checkpoint(func, "realize_annotations", config.interner)`

**Inside `emit_postprocess()` (postprocess.rs) -- needs interner parameter added:**
- [ ] After `verify()` (step 11): `trace_pipeline_checkpoint(func, "verify_final", interner)`
- [ ] After `check_fbip()` (step 12): `trace_pipeline_checkpoint(func, "fbip_enforcement", interner)`

### 05.1.3 Verify and test

- [ ] Use `info` level (not `debug` or `trace`) so events are captured by the shell driver without overwhelming verbosity. Note: this is intentionally different from the existing `trace_phase_snapshot` in `emit_unified.rs` which uses `trace!` on `ori_arc::aims::realize` for finer-grained realization-step snapshots. The `info`-level pipeline target is coarser-grained and designed for shell parsing.
- [ ] Add a Rust unit test verifying that the checkpoint function doesn't panic and produces the expected field names (use `tracing-test` subscriber if available in dependencies, or just verify the function is callable with a default `ArcFunction`)
- [ ] Verify: `ORI_LOG=ori_arc::aims::pipeline=info ori build diagnostics/fixtures/clean.ori 2>&1 | grep "AIMS phase checkpoint"` shows checkpoint lines covering all phases listed above (at least 15 checkpoints per function)
- [ ] Verify: checkpoint events include `function`, `rc_incs`, `rc_decs`, `blocks`, and `vars` fields
- [ ] Verify: multi-function fixture produces distinct `function=` values in checkpoint events, enabling per-function grouping
- [ ] Run `timeout 150 cargo t -p ori_arc` to verify no regressions

- [ ] **Subsection close-out (05.1)** -- MANDATORY before starting 05.2:
  - [ ] All tasks above are `[x]` and verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 05.2 Create bisect-passes.sh shell driver

**File(s):** `diagnostics/bisect-passes.sh` (new), `diagnostics/self-test.sh`, `diagnostics/README.md`

Create a shell script that runs a program with `ORI_LOG=ori_arc::aims::pipeline=info`, parses the checkpoint events, and reports which phase first introduces a problem.

### 05.2.1 Core script

- [ ] Create `diagnostics/bisect-passes.sh` with:
  - `--help`, `--no-color`, `--color` (standard options, consistent with other diagnostic scripts)
  - `--function <name>` -- filter to a specific function's phases (grep by function name in trace output)
  - `--rc-only` -- suppress structural metrics columns (show only RC data)
  - Input: a single `.ori` file
  - Source `_common.sh` for `find_ori_bin` binary discovery
  - Behavior:
    1. Create a temporary directory (`mktemp -d`) and set up `trap` cleanup on EXIT
    2. Compile with `ORI_LOG=ori_arc::aims::pipeline=info` and capture stderr to a temp file
    3. Build the binary to the temp directory: `ori build "$FILE" -o "$tmpdir/bisect-bin"` (never beside the input file)
    4. Parse checkpoint events from stderr to extract per-phase data: `function`, `phase`, `rc_incs`, `rc_decs`, `blocks`, `vars`
    5. Group events by function name, then display per-function tables with columns: `Phase | RC Incs | RC Decs | Balance | Blocks | Vars | Delta`
       - `Balance` = `rc_incs - rc_decs`
       - `Delta` = change in balance from previous phase
    6. Highlight the first phase where balance changes from 0 to non-zero (potential leak/over-release introduction)
    7. Highlight any phase where `blocks` or `vars` count changes significantly (structural transformation)
    8. Run the built binary from `$tmpdir` and check: does it crash? Does `ORI_CHECK_LEAKS=1 $tmpdir/bisect-bin` report leaks?
    9. Cleanup: `trap` removes `$tmpdir` on exit (no artifacts left beside input files)
  - Exit codes: 0 = all phases clean (no divergence detected), 1 = divergence detected, 2 = usage/infrastructure error
  - Must handle multi-function programs: show per-function tables (each function's phases are grouped by `function=` field from checkpoint events)
- [ ] Script must NOT regex-match LLVM IR for RC ops (that would be LEAK:scattered-knowledge per the plan's design principles). It reads the structured tracing output from the compiler's canonical checkpoint events.

### 05.2.2 Self-test entries

- [ ] Add self-test entries to `diagnostics/self-test.sh`:
  - `bisect-passes.sh --help` shows usage (contains "Usage:")
  - `bisect-passes.sh fixtures/simple.ori` runs without error (exit 0)
  - `bisect-passes.sh fixtures/clean.ori` shows phase table with RC counts (output contains "AIMS phase" or column headers)
  - `bisect-passes.sh --function main fixtures/clean.ori` filters to main function
- [ ] Add `bisect-passes.sh` to the fixture existence check section if applicable

### 05.2.3 Documentation

- [ ] Add `bisect-passes.sh` entry to `diagnostics/README.md` usage section with:
  - Purpose: "Identify which AIMS pipeline phase introduced an RC imbalance or structural change"
  - Example invocation and sample output
  - Workflow integration: "Use after `diagnose-aot.sh` identifies a leak or crash to narrow down to the specific pipeline phase"
- [ ] Verify: `diagnostics/self-test.sh` passes with the new entries

### 05.2.4 Cross-section plan update: Section 06 fixture coverage

Per `impl-hygiene.md`: "Cross-section fixes require cross-section plan updates." Section 06 was updated during plan review (2026-04-10) to include `bisect-passes.sh`:

- [x] Edit `plans/diagnostic-tooling-improvements/section-06-fixtures.md` — added `bisect-passes.sh` to Section 06.2's diagnostic script coverage matrix (2026-04-10 plan review)
- [x] Added explicit self-test items in Section 06.2 for `bisect-passes.sh` against fixtures (2026-04-10 plan review)
- [x] Added `bisect-passes.sh` to Section 06's success criteria (2026-04-10 plan review)

- [ ] **Subsection close-out (05.2)** -- MANDATORY before starting 05.R:
  - [ ] All tasks above are `[x]` and verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 05.R Third Party Review Findings

- [x] `[TPR-05-001-codex][high]` `section-05-bisect-passes.md:110` — Add function identity to AIMS checkpoint events.
  Resolved: Fixed on 2026-04-10. Added `interner: &ori_ir::StringInterner` parameter and `function = interner.lookup(func.name)` field to all checkpoint calls.
- [x] `[TPR-05-001-gemini][high]` `section-05-bisect-passes.md:67` — Include function name in trace checkpoints for multi-function bisection.
  Resolved: Fixed on 2026-04-10. Same fix as [TPR-05-001-codex] (functionally identical finding).
- [x] `[TPR-05-002-codex][medium]` `section-05-bisect-passes.md:222` — Anchor bisect-passes fixture coverage in Section 06.
  Resolved: Fixed on 2026-04-10. Changed 05.2.4 from passive note to mandatory cross-section plan update requiring direct edit of section-06-fixtures.md.
- [x] `[TPR-05-002-gemini][medium]` `section-05-bisect-passes.md:152` — Mandate explicit cross-section plan update.
  Resolved: Fixed on 2026-04-10. Same fix as [TPR-05-002-codex] (functionally identical finding).
- [x] `[TPR-05-003-codex][medium]` `section-05-bisect-passes.md:191` — Constrain bisect-passes builds to a temporary output path.
  Resolved: Fixed on 2026-04-10. Added tmpdir/mktemp/trap cleanup requirements to shell script behavior spec.
- [x] `[TPR-05-004-codex][low]` `section-05-bisect-passes.md:129` — Correct the checkpoint tracing target contract.
  Resolved: Fixed on 2026-04-10. Corrected precedent claim (trace_phase_snapshot uses trace!, not info!), documented new target as intentionally different, added arc.md and CLAUDE.md doc update items to completion checklist.

**Iteration 2 findings:**
- [x] `[TPR-05-005-codex][medium]` `section-06-fixtures.md:42` — Add bisect-passes to Section 06 fixture coverage.
  Resolved: Fixed on 2026-04-10. Directly edited section-06-fixtures.md: added bisect-passes.sh to success criteria and self-test matrix.
- [x] `[TPR-05-006-codex][low]` `section-07-integration.md:136` — Add pipeline tracing target docs to Section 07.
  Resolved: Fixed on 2026-04-10. Added ori_arc::aims::pipeline tracing target doc items to Section 07.4 remaining tasks.

---

## 05.N Completion Checklist

- [ ] All subsections (05.PRE, 05.1, 05.2) complete
- [ ] `aims_pipeline.rs` has been split and no submodule exceeds 300 lines
- [ ] `timeout 150 cargo t -p ori_arc` passes
- [ ] `timeout 150 cargo cl -p ori_arc` passes (clippy clean)
- [ ] `diagnostics/self-test.sh` passes (including new bisect-passes.sh entries)
- [ ] `timeout 150 ./test-all.sh` green -- no regressions
- [ ] **Doc update (SSOT):**
  - [ ] Add `bisect-passes.sh` to `@diagnostic.md` (`.claude/rules/diagnostic.md`) Diagnostic Scripts table with flags (`--function`, `--rc-only`, `--no-color`, `--color`)
  - [ ] Add `bisect-passes.sh` to `diagnostics/README.md` usage section and workflow
  - [ ] Update CLAUDE.md Diagnostic scripts reference if it lists scripts explicitly
  - [ ] Add `ori_arc::aims::pipeline` tracing target to `.claude/rules/arc.md` §Debugging section (alongside existing `ori_arc::aims::realize` target documentation)
  - [ ] Update CLAUDE.md §Tracing with `=ori_arc::aims::pipeline=info` example for pipeline bisection
- [x] **Cross-section:** Section 06 updated — `bisect-passes.sh` added to success criteria and self-test matrix (done during plan review 2026-04-10)
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review` passed
- [ ] **`/improve-tooling` section-close sweep**
